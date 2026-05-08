use std::{mem, ops::Range};

use lsp_types::{
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams,
};
use rustc_hash::FxHashSet;
use triomphe::Arc;
use utils::{
    line_index::LineIndex,
    lines::{LineEnding, LineInfo, PositionEncoding},
};
use vfs::{VfsPath, loader::LoadResult};

use crate::{
    DEFAULT_PROCESS_NAME,
    config::user_config::DiagnosticsUpdateUserConfig,
    global_state::{GlobalState, process_changes::DiagnosticInvalidation, reload},
    lsp_ext::from_proto,
};

pub(crate) fn handle_cancel(
    state: &mut GlobalState,
    params: lsp_types::CancelParams,
) -> anyhow::Result<()> {
    let id: lsp_server::RequestId = match params.id {
        lsp_types::NumberOrString::Number(id) => id.into(),
        lsp_types::NumberOrString::String(id) => id.into(),
    };
    state.cancel(id);
    Ok(())
}

pub(crate) fn handle_did_open_text_document(
    state: &mut GlobalState,
    params: DidOpenTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        let file_id = set_vfs_file_contents(state, &path, params.text_document.text.clone());
        if state
            .mem_docs
            .insert(file_id, path.clone(), params.text_document.version, params.text_document.text)
            .is_some()
        {
            tracing::error!("duplicate DidOpenTextDocument: {}", path);
        }
    }
    Ok(())
}

pub(crate) fn handle_did_change_text_document(
    state: &mut GlobalState,
    params: DidChangeTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        let data = match state.mem_docs.get_mut_by_path(&path) {
            Some(doc) => {
                // The version in DidChangeTextDocument is the one after all edits,
                // so we should apply it before the vfs is notified.
                doc.version = params.text_document.version;
                &mut doc.data
            }
            None => {
                tracing::error!("unexpected DidChangeTextDocument: {}", path);
                return Ok(());
            }
        };

        let text = apply_document_changes(
            state.config.position_encoding(),
            mem::take(data),
            params.content_changes,
        );

        if *data != text {
            *data = text.clone();
            set_vfs_file_contents(state, &path, text);
        }
    }
    Ok(())
}

pub(crate) fn handle_did_close_text_document(
    state: &mut GlobalState,
    params: DidCloseTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        if state.mem_docs.remove_path(&path).is_none() {
            tracing::error!("orphan DidCloseTextDocument: {}", path);
        }

        if let Some(path) = path.as_abs_path() {
            state.vfs_loader.handle.invalidate(path.to_path_buf());
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
        // Re-fetch workspaces if a workspace related file has changed
        state.fetch_workspaces_task.request(format!("DidSaveTextDocument {abs_path}"));
    }

    if state.config.user_config.diagnostics.update == DiagnosticsUpdateUserConfig::OnSave
        && let Ok(file_id) = state.make_snapshot().file_id(&params.text_document.uri)
    {
        state.invalidate_diagnostics(DiagnosticInvalidation::FileChanges(FxHashSet::from_iter([
            file_id,
        ])));
    }

    Ok(())
}

pub(crate) fn handle_did_change_configuration(
    state: &mut GlobalState,
    // As stated in https://github.com/microsoft/language-server-protocol/issues/676,
    // this notification's parameters should be ignored and the actual config queried separately.
    _params: DidChangeConfigurationParams,
) -> anyhow::Result<()> {
    state.send_request::<lsp_types::request::WorkspaceConfiguration>(
        lsp_types::ConfigurationParams {
            items: vec![lsp_types::ConfigurationItem {
                scope_uri: None,
                section: Some(DEFAULT_PROCESS_NAME.into()),
            }],
        },
        |this, resp| {
            tracing::debug!("config update response: '{:?}", resp);
            let lsp_server::Response { result, error, .. } = resp;

            match (result, error) {
                (_, Some(err)) => {
                    tracing::error!("failed to fetch the server settings: {:?}", err)
                }
                (Some(mut configs), None) => {
                    if let Some(json) = configs.get_mut(0) {
                        // Note that json can be null according to the spec if the client can't
                        // provide a configuration. This is handled in Config::update below.
                        let mut config = (*this.config).clone();
                        this.config_errors = config.update(json.take()).err();
                        this.update_configuration(config);
                    }
                }
                (None, None) => {
                    tracing::error!("received empty server settings response from the client")
                }
            }
        },
    );

    Ok(())
}

pub(crate) fn handle_did_change_workspace_folders(
    state: &mut GlobalState,
    params: DidChangeWorkspaceFoldersParams,
) -> anyhow::Result<()> {
    let config = Arc::make_mut(&mut state.config);

    for workspace in params.event.removed {
        if let Ok(path) = from_proto::abs_path(&workspace.uri) {
            config.remove_workspace(&path);
        }
    }

    let added = params.event.added.into_iter().filter_map(|it| from_proto::abs_path(&it.uri).ok());
    config.add_workspaces(added);

    // TODO: ??
    config.rediscover_manifest();
    state.fetch_workspaces_task.request("client workspaces changed".to_string());

    Ok(())
}

pub(crate) fn handle_did_change_watched_files(
    state: &mut GlobalState,
    params: DidChangeWatchedFilesParams,
) -> anyhow::Result<()> {
    for change in params.changes {
        if let Ok(path) = from_proto::abs_path(&change.uri) {
            // invalidate the file in the VFS so that it's reloaded later
            state.vfs_loader.handle.invalidate(path);
        }
    }
    Ok(())
}

fn set_vfs_file_contents(state: &mut GlobalState, path: &VfsPath, text: String) -> vfs::FileId {
    let (text, endings) = LineEnding::normalize(text);
    let mut vfs = state.vfs.write();
    vfs.0.set_file_contents(path, LoadResult::Loaded(text, endings));
    vfs.0.file_id(path).expect("loaded file must have a FileId")
}

fn apply_document_changes(
    encoding: PositionEncoding,
    file_contents: String,
    content_changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) -> String {
    // Skip to the last full document change and peek at the first content change
    let (mut text, content_changes) = {
        match content_changes.iter().rposition(|change| change.range.is_none()) {
            Some(idx) => {
                let (text, rest) = content_changes.split_at(idx + 1);
                (text[idx].text.clone(), rest)
            }
            None => (file_contents.clone(), &content_changes[..]),
        }
    };

    if content_changes.is_empty() {
        return text;
    }

    // The changes can cross lines so we have to keep our line index updated.
    // Here's an optimization: we only rebuild the index if we have to, iff
    // the change's start line is greater than the last valid line.
    // The VFS will normalize the end of lines to `\n`.
    let mut line_info = LineInfo {
        index: Arc::new(LineIndex::new(&text)),
        // We don't care about line endings here.
        ending: LineEnding::Unix,
        encoding,
    };

    // set to infinity at first, to avoid rebuilding the index on the first change
    let mut index_valid_until = !0u32;
    for change in content_changes {
        // The None case can't happen
        let range = change.range.unwrap();
        if index_valid_until <= range.end.line {
            *Arc::make_mut(&mut line_info.index) = LineIndex::new(&text);
        }
        index_valid_until = range.start.line;
        if let Ok(range) = from_proto::text_range(&line_info, range) {
            text.replace_range(Range::<usize>::from(range), &change.text);
        }
    }
    text
}
