use std::ops::Range;

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
use vfs::{FileId, VfsPath, loader::LoadResult};

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

pub(crate) fn handle_work_done_progress_cancel(
    state: &mut GlobalState,
    params: lsp_types::WorkDoneProgressCancelParams,
) -> anyhow::Result<()> {
    state.cancel_work_done_progress(params);
    Ok(())
}

pub(crate) fn handle_did_open_text_document(
    state: &mut GlobalState,
    params: DidOpenTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        let file_id = open_vfs_file_contents(state, &path, &params.text_document.text)?;
        if state.mem_docs.text(file_id).is_some_and(|text| text != params.text_document.text) {
            tracing::warn!(
                ?file_id,
                path = %path,
                "open document alias has different text; keeping canonical analysis buffer"
            );
        }
        if state.mem_docs.insert(
            file_id,
            path.clone(),
            params.text_document.version,
            params.text_document.text,
        ) {
            tracing::error!("duplicate DidOpenTextDocument: {}", path);
        }
        state.pending_document_diagnostic_targets.insert(file_id);
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
        let text = match state.mem_docs.text_for_change(&path, file_id) {
            Some(text) => text.to_owned(),
            None => {
                tracing::error!("unexpected DidChangeTextDocument: {}", path);
                return Ok(());
            }
        };

        let text = match update_document_text(
            state.config.position_encoding(),
            &text,
            params.content_changes,
        ) {
            Ok(text) => text,
            Err(error) => {
                tracing::error!("invalid DidChangeTextDocument for {path}: {error:#}");
                return Ok(());
            }
        };
        if !state.mem_docs.apply_change(&path, file_id, params.text_document.version, text.clone())
        {
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
        let file_id = state.mem_docs.file_id(&path);
        if !state.mem_docs.remove_path(&path) {
            tracing::error!("orphan DidCloseTextDocument: {}", path);
        }
        if let Some(file_id) = file_id {
            state.pending_document_diagnostic_targets.insert(file_id);
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
        let config = Arc::make_mut(&mut state.config);
        config.refresh_project_manifests();
        state.request_workspace_auto_reload(format!("DidSaveTextDocument {abs_path}"));
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
    config.refresh_project_manifests();
    state.request_workspace_reload("client workspaces changed");

    Ok(())
}

pub(crate) fn handle_did_change_watched_files(
    state: &mut GlobalState,
    params: DidChangeWatchedFilesParams,
) -> anyhow::Result<()> {
    let mut workspace_structure_change = None;

    for change in params.changes {
        if let Ok(path) = from_proto::abs_path(&change.uri) {
            if reload::should_refresh_for_change(
                &path,
                change.typ != lsp_types::FileChangeType::CHANGED,
            ) {
                workspace_structure_change.get_or_insert(path);
                continue;
            }

            // invalidate the file in the VFS so that it's reloaded later
            state.vfs_loader.handle.invalidate(path);
        }
    }

    if let Some(path) = workspace_structure_change {
        let config = Arc::make_mut(&mut state.config);
        config.refresh_project_manifests();
        state.request_workspace_auto_reload(format!("DidChangeWatchedFiles {path}"));
    }

    Ok(())
}

pub(crate) fn handle_set_trace(
    state: &mut GlobalState,
    params: lsp_types::SetTraceParams,
) -> anyhow::Result<()> {
    state.set_lsp_trace(params.value);
    Ok(())
}

fn set_vfs_file_contents(
    state: &mut GlobalState,
    path: &VfsPath,
    text: String,
) -> anyhow::Result<vfs::FileId> {
    let (text, endings) = LineEnding::normalize(text);
    let mut vfs = state.vfs.write();
    vfs.0.set_file_contents(path, LoadResult::Loaded(text, endings));
    vfs.0.file_id(path).ok_or_else(|| anyhow::format_err!("loaded file has no FileId: {path}"))
}

fn open_vfs_file_contents(
    state: &mut GlobalState,
    path: &VfsPath,
    text: &str,
) -> anyhow::Result<vfs::FileId> {
    let mut vfs = state.vfs.write();
    let file_id = vfs.0.register_file_ingress(path);
    if state.mem_docs.contains_file_id(file_id) {
        return Ok(file_id);
    }

    let (text, endings) = LineEnding::normalize(text.to_owned());
    vfs.0.set_file_contents(path, LoadResult::Loaded(text, endings));
    vfs.0.file_id(path).ok_or_else(|| anyhow::format_err!("loaded file has no FileId: {path}"))
}

fn open_mem_doc_file_id(state: &GlobalState, path: &VfsPath) -> Option<FileId> {
    state.mem_docs.file_id(path).or_else(|| {
        state.vfs.read().0.file_id(path).filter(|file_id| state.mem_docs.contains_file_id(*file_id))
    })
}

fn update_document_text(
    encoding: PositionEncoding,
    data: &str,
    content_changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) -> anyhow::Result<Option<String>> {
    let text = apply_document_changes(encoding, data, content_changes)?;

    if data == text { Ok(None) } else { Ok(Some(text)) }
}

fn apply_document_changes(
    encoding: PositionEncoding,
    file_contents: &str,
    content_changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) -> anyhow::Result<String> {
    // Skip to the last full document change and peek at the first content change
    let (mut text, content_changes) = {
        match content_changes.iter().rposition(|change| change.range.is_none()) {
            Some(idx) => {
                let (full_doc_changes, rest) = content_changes.split_at(idx + 1);
                match full_doc_changes.last() {
                    Some(full_doc_change) => (full_doc_change.text.clone(), rest),
                    None => (file_contents.to_owned(), rest),
                }
            }
            None => (file_contents.to_owned(), &content_changes[..]),
        }
    };

    if content_changes.is_empty() {
        return Ok(text);
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
        let Some(range) = change.range else {
            text = change.text.clone();
            *Arc::make_mut(&mut line_info.index) = LineIndex::new(&text);
            index_valid_until = !0u32;
            continue;
        };
        if index_valid_until <= range.end.line {
            *Arc::make_mut(&mut line_info.index) = LineIndex::new(&text);
        }
        index_valid_until = range.start.line;
        let range = from_proto::text_range(&line_info, range)?;
        text.replace_range(Range::<usize>::from(range), &change.text);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lsp_server::Connection;
    use lsp_types::{
        DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidOpenTextDocumentParams,
        FileChangeType, FileEvent, SetTraceParams, TextDocumentContentChangeEvent,
        TextDocumentItem, TraceValue, Url, VersionedTextDocumentIdentifier,
    };
    use project_model::project_manifest;
    use utils::{lines::PositionEncoding, paths::AbsPathBuf, test_support::TestDir};
    use vfs::VfsPath;

    use super::{
        handle_did_change_text_document, handle_did_change_watched_files,
        handle_did_open_text_document, handle_set_trace, update_document_text,
    };
    use crate::{
        Opt,
        config::{self, user_config::UserConfig},
        global_state::GlobalState,
        i18n::I18n,
    };

    fn test_state() -> (GlobalState, Connection) {
        let root_path = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap());
        test_state_with_root(root_path)
    }

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
    fn clearing_document_updates_mem_doc_and_vfs_text() {
        let text = "module top;\nendmodule\n".to_owned();
        let vfs_text = update_document_text(
            PositionEncoding::Utf8,
            &text,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: String::new(),
            }],
        )
        .unwrap();

        assert_eq!(vfs_text.as_deref(), Some(""));
    }

    #[test]
    fn unchanged_document_skips_vfs_update() {
        let text = "module top;\nendmodule\n".to_owned();
        let vfs_text = update_document_text(
            PositionEncoding::Utf8,
            &text,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "module top;\nendmodule\n".to_owned(),
            }],
        )
        .unwrap();

        assert!(vfs_text.is_none());
    }

    #[test]
    fn invalid_range_change_does_not_apply_partial_text() {
        let text = "module top;\nendmodule\n".to_owned();
        let result = update_document_text(
            PositionEncoding::Utf8,
            &text,
            vec![TextDocumentContentChangeEvent {
                range: Some(lsp_types::Range::new(
                    lsp_types::Position::new(99, 0),
                    lsp_types::Position::new(99, 1),
                )),
                range_length: None,
                text: "broken".to_owned(),
            }],
        );

        assert!(result.is_err());
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
        let file_id = state.mem_docs.file_id(&vfs_path).unwrap();

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

        assert_eq!(state.mem_docs.version_for_path(&vfs_path), Some(1));
        assert_eq!(state.mem_docs.text(file_id), Some("module top;\nendmodule\n"));
    }

    #[test]
    fn divergent_alias_did_change_does_not_update_canonical_buffer() {
        let root = TestDir::new("divergent-alias-change");
        let (mut state, _client) = test_state_with_root(root.path().to_path_buf());
        let source_path = root.join("workspace/top.sv");
        let alias_path = root.join("alias/top.sv");
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(alias_path.parent().unwrap()).unwrap();
        std::fs::write(&source_path, "module top;\nendmodule\n").unwrap();
        std::fs::hard_link(&source_path, &alias_path).unwrap();
        let source_uri = Url::from_file_path(source_path.as_path()).unwrap();
        let alias_uri = Url::from_file_path(alias_path.as_path()).unwrap();
        let source_vfs_path = VfsPath::from(source_path);
        let alias_vfs_path = VfsPath::from(alias_path);

        handle_did_open_text_document(
            &mut state,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: source_uri,
                    language_id: "systemverilog".to_owned(),
                    version: 1,
                    text: "module top;\nendmodule\n".to_owned(),
                },
            },
        )
        .unwrap();
        let file_id = state.mem_docs.file_id(&source_vfs_path).unwrap();
        handle_did_open_text_document(
            &mut state,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: alias_uri.clone(),
                    language_id: "systemverilog".to_owned(),
                    version: 12,
                    text: "module broken(;\nendmodule\n".to_owned(),
                },
            },
        )
        .unwrap();

        assert_eq!(state.mem_docs.file_id(&alias_vfs_path), Some(file_id));
        assert_eq!(state.mem_docs.text(file_id), Some("module top;\nendmodule\n"));
        assert_eq!(state.mem_docs.open_documents(file_id).len(), 1);

        handle_did_change_text_document(
            &mut state,
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri: alias_uri, version: 13 },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "module still_broken(;\nendmodule\n".to_owned(),
                }],
            },
        )
        .unwrap();

        assert_eq!(state.mem_docs.version_for_path(&alias_vfs_path), Some(12));
        assert_eq!(state.mem_docs.text(file_id), Some("module top;\nendmodule\n"));
    }

    #[test]
    fn set_trace_notification_updates_server_trace_level() {
        let (mut state, client) = test_state();

        handle_set_trace(&mut state, SetTraceParams { value: TraceValue::Verbose }).unwrap();

        assert_eq!(state.lsp_trace.level(), TraceValue::Verbose);
        assert!(client.receiver.recv_timeout(Duration::from_millis(50)).is_err());
    }

    #[test]
    fn watched_manifest_delete_requests_workspace_reload() {
        let root = TestDir::new("watched-manifest-delete");
        let (mut state, _client) = test_state_with_root(root.path().to_path_buf());
        let manifest_path = root.join(project_manifest::MANIFEST_FILE_NAME);
        let manifest_uri = Url::from_file_path(manifest_path.as_path()).unwrap();

        handle_did_change_watched_files(
            &mut state,
            DidChangeWatchedFilesParams {
                changes: vec![FileEvent::new(manifest_uri, FileChangeType::DELETED)],
            },
        )
        .unwrap();

        assert!(state.fetch_workspaces_task.has_op_requested());
    }
}
