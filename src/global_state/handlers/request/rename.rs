use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{from_proto, to_proto},
};

pub(crate) fn handle_prepare_rename(
    snap: GlobalStateSnapshot,
    params: lsp_types::TextDocumentPositionParams,
) -> anyhow::Result<Option<lsp_types::PrepareRenameResponse>> {
    let position = from_proto::file_position(&snap, params)?;
    let config = snap.rename_config(position.file_id);
    let line_index = snap.line_info(position.file_id)?;

    let text_range = snap
        .analysis
        .prepare_rename(position, config)?
        .map_err(|err| to_proto::rename_error(snap.config.i18n, err))?;
    let range = to_proto::range(&line_index, text_range);
    Ok(Some(lsp_types::PrepareRenameResponse::Range(range)))
}

pub(crate) fn handle_rename(
    snap: GlobalStateSnapshot,
    params: lsp_types::RenameParams,
) -> anyhow::Result<Option<lsp_types::WorkspaceEdit>> {
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.rename_config(position.file_id);
    let change = snap
        .analysis
        .rename(position, config, &params.new_name)?
        .map_err(|err| to_proto::rename_error(snap.config.i18n, err))?;

    let workspace_edit = to_proto::workspace_edit(&snap, change)?;
    Ok(Some(workspace_edit))
}
