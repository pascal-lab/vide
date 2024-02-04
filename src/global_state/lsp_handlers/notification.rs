use std::{mem, ops::Range};

use line_index::LineIndex;
use lsp_types::{
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams,
};
use triomphe::Arc;
use utils::{
    lines::{LineEnding, LineInfo, PositionEncoding},
    text_edit::{SourceEdit, SourceEditKind, SourcePoint},
};

use crate::{
    global_state::{mem_docs::DocumentData, reload, GlobalState},
    lsp_ext::from_proto,
    DEFAULT_PROCESS_NAME,
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
        let data = DocumentData {
            version: params.text_document.version,
            data: params.text_document.text.clone(),
        };
        if state.mem_docs.insert(path.clone(), data).is_some() {
            tracing::error!("duplicate DidOpenTextDocument: {}", path);
        }

        let (text, line_ending) = LineEnding::normalize(params.text_document.text);
        state.vfs.write().0.set_file_contents(&path, Ok((text, line_ending)), SourceEditKind::Full);
    }
    Ok(())
}

pub(crate) fn handle_did_change_text_document(
    state: &mut GlobalState,
    params: DidChangeTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        let data = match state.mem_docs.get_mut(&path) {
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

        let (text, edits) = apply_document_changes(
            state.config.position_encoding(),
            mem::take(data),
            params.content_changes,
        );
        // TODO: we can calculate the line ending from the edits to speed up
        let (text, line_ending) = LineEnding::normalize(text);

        *data = text.clone();

        state.vfs.write().0.set_file_contents(&path, Ok((text, line_ending)), edits);
    }
    Ok(())
}

pub(crate) fn handle_did_close_text_document(
    state: &mut GlobalState,
    params: DidCloseTextDocumentParams,
) -> anyhow::Result<()> {
    if let Ok(path) = from_proto::vfs_path(&params.text_document.uri) {
        if state.mem_docs.remove(&path).is_none() {
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
        state.fetch_workspaces_task.request(format!("DidSaveTextDocument {abs_path}"), ());
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

    if config.detached_files.is_empty() {
        config.rediscover_manifest();
        state.fetch_workspaces_task.request("client workspaces changed".to_string(), ())
    }

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

pub(crate) fn handle_workspace_reload(state: &mut GlobalState, _: ()) -> anyhow::Result<()> {
    state.fetch_workspaces_task.request("reload workspace request".to_string(), ());
    Ok(())
}

fn apply_document_changes(
    encoding: PositionEncoding,
    file_contents: String,
    mut content_changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) -> (String, SourceEditKind) {
    // Skip to the last full document change and peek at the first content change
    let (mut text, content_changes) = {
        let last_full_change = content_changes.iter().rposition(|change| change.range.is_none());
        if let Some(idx) = last_full_change {
            (mem::take(&mut content_changes[idx].text), &content_changes[idx + 1..])
        } else {
            (file_contents, &content_changes[..])
        }
    };

    if content_changes.is_empty() {
        return (text, SourceEditKind::Edits(vec![]));
    }

    // The changes can cross lines so we have to keep our line index updated.
    // Here's an optimization: we only rebuild the index if we have to, iff
    // the change's start line is greater than the last valid line.
    // The VFS will normalize the end of lines to `\n`.
    // TODO: make line_index incremental?
    let mut line_index = LineInfo {
        index: Arc::new(LineIndex::new(&text)),
        // We don't care about line endings here.
        ending: LineEnding::Unix,
        encoding,
    };

    let mut edits = vec![];

    // set to infinity at first, to avoid rebuilding the index on the first change
    let mut index_valid_until = !0u32;
    for change in content_changes {
        // The None case can't happen
        let range = change.range.unwrap();
        if index_valid_until <= range.end.line {
            *Arc::make_mut(&mut line_index.index) = LineIndex::new(&text);
        }
        index_valid_until = range.start.line;
        // TODO: Use rope for better performance?
        if let Ok(range) = from_proto::text_range(&line_index, range) {
            // TODO: The positions is not correct, but it doesn't matter for now.
            // Maybe we should fix it?
            let range = Range::<usize>::from(range);
            edits.push(SourceEdit {
                start_byte: range.start,
                old_end_byte: range.end,
                new_end_byte: range.start + change.text.len(),
                start_position: SourcePoint { row: 0, column: 0 },
                old_end_position: SourcePoint { row: 0, column: 0 },
                new_end_position: SourcePoint { row: 0, column: 0 },
            });
            text.replace_range(range, &change.text);
        }
    }
    (text, SourceEditKind::Edits(edits))
}
