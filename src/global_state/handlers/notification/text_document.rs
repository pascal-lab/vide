use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams,
};
use rustc_hash::FxHashSet;
use triomphe::Arc;

use super::text_changes::{
    open_mem_doc_file_id, open_vfs_file_contents, set_vfs_file_contents, update_document_text,
};
use crate::{
    config::user_config::DiagnosticsUpdateUserConfig,
    global_state::{GlobalState, process_changes::DiagnosticInvalidation, reload},
    lsp_ext::from_proto,
};

pub(crate) fn handle_did_open_text_document(
    state: &mut GlobalState,
    params: DidOpenTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        let file_id = open_vfs_file_contents(state, &path, &params.text_document.text)?;
        if state
            .analysis
            .mem_docs
            .text(file_id)
            .is_some_and(|text| text != params.text_document.text)
        {
            tracing::warn!(
                ?file_id,
                path = %path,
                "open document alias has different text; keeping canonical analysis buffer"
            );
        }
        if state.analysis.mem_docs.insert(
            file_id,
            path.clone(),
            params.text_document.version,
            params.text_document.text,
        ) {
            tracing::error!("duplicate DidOpenTextDocument: {}", path);
        }
        state.diagnostics.pending_document_diagnostic_targets.insert(file_id);
    }
    Ok(())
}

pub(crate) fn handle_did_change_text_document(
    state: &mut GlobalState,
    params: DidChangeTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        let Some(file_id) = open_mem_doc_file_id(state, &path) else {
            tracing::error!("unexpected DidChangeTextDocument: {}", path);
            return Ok(());
        };
        let text = match state.analysis.mem_docs.text_for_change(&path, file_id) {
            Some(text) => text.to_owned(),
            None => {
                tracing::error!("unexpected DidChangeTextDocument: {}", path);
                return Ok(());
            }
        };

        let text = match update_document_text(
            state.config_state.config.position_encoding(),
            &text,
            params.content_changes,
        ) {
            Ok(text) => text,
            Err(error) => {
                tracing::error!("invalid DidChangeTextDocument for {path}: {error:#}");
                return Ok(());
            }
        };
        if !state.analysis.mem_docs.apply_change(
            &path,
            file_id,
            params.text_document.version,
            text.clone(),
        ) {
            tracing::error!("unexpected DidChangeTextDocument: {}", path);
            return Ok(());
        }
        if let Some(text) = text {
            set_vfs_file_contents(state, &path, text)?;
        }
    }
    Ok(())
}

pub(crate) fn handle_did_close_text_document(
    state: &mut GlobalState,
    params: DidCloseTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        let file_id = state.analysis.mem_docs.file_id(&path);
        if !state.analysis.mem_docs.remove_path(&path) {
            tracing::error!("orphan DidCloseTextDocument: {}", path);
        }
        if let Some(file_id) = file_id {
            state.diagnostics.pending_document_diagnostic_targets.insert(file_id);
        }

        if let Some(path) = path.as_abs_path() {
            state.workspace.vfs_loader.handle.invalidate(path.to_path_buf());
        }
    }
    Ok(())
}

pub(crate) fn handle_did_save_text_document(
    state: &mut GlobalState,
    params: DidSaveTextDocumentParams,
) -> anyhow::Result<()> {
    // TODO: check on save
    if let Ok(vfs_path) = from_proto::vfs_path(&params.text_document.uri)
        && let Some(abs_path) = vfs_path.as_abs_path()
        && reload::should_refresh_for_change(abs_path, false)
    {
        // Re-fetch workspaces if a workspace related file has changed.
        let config = Arc::make_mut(&mut state.config_state.config);
        config.refresh_project_manifests();
        state.request_workspace_auto_reload(format!("DidSaveTextDocument {abs_path}"));
    }

    if state.config_state.config.user_config.diagnostics.update
        == DiagnosticsUpdateUserConfig::OnSave
        && let Ok(file_id) = state.make_snapshot().file_id(&params.text_document.uri)
    {
        state.invalidate_diagnostics(DiagnosticInvalidation::FileChanges(FxHashSet::from_iter([
            file_id,
        ])));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use lsp_server::Connection;
    use lsp_types::{
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, TextDocumentContentChangeEvent,
        TextDocumentItem, TraceValue, Url, VersionedTextDocumentIdentifier,
    };
    use utils::{paths::AbsPathBuf, test_support::TestDir};
    use vfs::VfsPath;

    use super::{handle_did_change_text_document, handle_did_open_text_document};
    use crate::{
        Opt,
        config::{self, user_config::UserConfig},
        global_state::GlobalState,
        i18n::I18n,
    };

    fn test_state_with_root(root_path: AbsPathBuf) -> (GlobalState, Connection) {
        let config = config::Config::new(
            Opt {
                process_name: "vide-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            lsp_types::ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );

        let (server, client) = Connection::memory();
        (GlobalState::new(server.sender, config, TraceValue::Off), client)
    }

    #[test]
    fn invalid_did_change_keeps_open_document_version_and_text() {
        let root = TestDir::new("invalid-did-change");
        let (mut state, _client) = test_state_with_root(root.path().to_path_buf());
        let file_path = root.join("top.sv");
        let uri = Url::from_file_path(file_path.as_path()).unwrap();
        let vfs_path = VfsPath::from(file_path);

        handle_did_open_text_document(
            &mut state,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "systemverilog".to_owned(),
                    version: 1,
                    text: "module top;\nendmodule\n".to_owned(),
                },
            },
        )
        .unwrap();
        let file_id = state.analysis.mem_docs.file_id(&vfs_path).unwrap();

        handle_did_change_text_document(
            &mut state,
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri, version: 2 },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: Some(lsp_types::Range::new(
                        lsp_types::Position::new(99, 0),
                        lsp_types::Position::new(99, 1),
                    )),
                    range_length: None,
                    text: "broken".to_owned(),
                }],
            },
        )
        .unwrap();

        assert_eq!(state.analysis.mem_docs.version_for_path(&vfs_path), Some(1));
        assert_eq!(state.analysis.mem_docs.text(file_id), Some("module top;\nendmodule\n"));
    }
}
