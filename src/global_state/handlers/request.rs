use ide::references::References;
use itertools::Itertools;
use lsp_types::{PrepareRenameResponse, RenameParams, WorkspaceEdit};
use span::FileRange;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{from_proto, to_proto},
};

pub(crate) fn handle_goto_definition(
    snap: GlobalStateSnapshot,
    params: lsp_types::GotoDefinitionParams,
) -> anyhow::Result<Option<lsp_types::GotoDefinitionResponse>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let Some(nav_info) = snap.analysis.goto_definition(position)? else {
        return Ok(None);
    };

    let src = FileRange { file_id: position.file_id, range: nav_info.range };
    let res = to_proto::goto_definition_response(&snap, Some(src), nav_info.info)?;
    Ok(Some(res))
}

pub(crate) fn handle_document_symbol(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentSymbolParams,
) -> anyhow::Result<Option<lsp_types::DocumentSymbolResponse>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let line_info = snap.line_info(file_id)?;
    let symbols = snap.analysis.document_symbol(file_id)?;

    let res = if snap.config.hierarchical_symbols() {
        symbols
            .into_iter()
            .map(|symbol| to_proto::document_symbol(&line_info, symbol))
            .collect_vec()
            .into()
    } else {
        let mut res = Vec::new();
        let url = to_proto::url(&snap, file_id);
        symbols.into_iter().for_each(|symbol| {
            to_proto::document_symbol_information(symbol, url.clone(), &line_info, &mut res);
        });
        res.into()
    };

    Ok(Some(res))
}

pub(crate) fn handle_document_highlight(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentHighlightParams,
) -> anyhow::Result<Option<Vec<lsp_types::DocumentHighlight>>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let line_info = snap.line_info(position.file_id)?;
    let config = snap.config.document_highlight_config();
    let Some(highlights) = snap.analysis.document_highlight(position, config)? else {
        return Ok(None);
    };

    let res = highlights
        .into_iter()
        .map(|highlight| to_proto::document_highlight(&line_info, highlight))
        .collect();
    Ok(Some(res))
}

pub(crate) fn handle_references(
    snap: GlobalStateSnapshot,
    params: lsp_types::ReferenceParams,
) -> anyhow::Result<Option<Vec<lsp_types::Location>>> {
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.config.references_config();
    let Some(refs) = snap.analysis.references(position, config)? else {
        return Ok(None);
    };

    let locations = refs
        .into_iter()
        .flat_map(|References { def, refs }| {
            let decl = def
                .into_iter()
                .flatten()
                .map(|nav| FileRange { file_id: nav.file_id, range: nav.focus_or_full_range() });

            let refs = refs.into_iter().flat_map(|(file_id, refs)| {
                refs.into_iter().map(move |(range, _)| FileRange { file_id, range })
            });

            decl.chain(refs)
        })
        .unique()
        .filter_map(|frange| to_proto::location(&snap, frange).ok())
        .collect_vec();

    Ok(Some(locations))
}

pub(crate) fn handle_prepare_rename(
    snap: GlobalStateSnapshot,
    params: lsp_types::TextDocumentPositionParams,
) -> anyhow::Result<Option<PrepareRenameResponse>> {
    let position = from_proto::file_position(&snap, params)?;
    let line_index = snap.line_info(position.file_id)?;

    let text_range = snap.analysis.prepare_rename(position)?.map_err(to_proto::rename_error)?;
    let range = to_proto::range(&line_index, text_range);
    Ok(Some(PrepareRenameResponse::Range(range)))
}

pub(crate) fn handle_rename(
    snap: GlobalStateSnapshot,
    params: RenameParams,
) -> anyhow::Result<Option<WorkspaceEdit>> {
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.config.rename_config();
    let change = snap
        .analysis
        .rename(position, config, &params.new_name)?
        .map_err(to_proto::rename_error)?;

    let workspace_edit = to_proto::workspace_edit(&snap, change)?;
    Ok(Some(workspace_edit))
}

pub(crate) fn handle_formatting(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentFormattingParams,
) -> anyhow::Result<Option<Vec<lsp_types::TextEdit>>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let line_info = snap.line_info(file_id)?;

    let config = snap.config.fmt_config();
    let edit =
        snap.analysis.format(file_id, None, &line_info, config)?.map_err(to_proto::format_error)?;

    let text_edits = edit.map(|edit| to_proto::text_edits(&line_info, edit));
    Ok(text_edits)
}

pub(crate) fn handle_range_formatting(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentRangeFormattingParams,
) -> anyhow::Result<Option<Vec<lsp_types::TextEdit>>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let line_info = snap.line_info(file_id)?;
    let line_ranges = Some((params.range.start.line as usize, params.range.end.line as usize));

    let config = snap.config.fmt_config();
    let edit = snap
        .analysis
        .format(file_id, line_ranges, &line_info, config)?
        .map_err(to_proto::format_error)?;

    let text_edits = edit.map(|edit| to_proto::text_edits(&line_info, edit));
    Ok(text_edits)
}

pub(crate) fn handle_on_type_formatting(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentOnTypeFormattingParams,
) -> anyhow::Result<Option<Vec<lsp_types::TextEdit>>> {
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let line_info = snap.line_info(position.file_id)?;

    let config = snap.config.fmt_config();
    let edit = snap
        .analysis
        .format_on_type(position, params.ch, &line_info, config)?
        .map_err(to_proto::format_error)?;

    let text_edits = edit.map(|edit| to_proto::text_edits(&line_info, edit));
    Ok(text_edits)
}
