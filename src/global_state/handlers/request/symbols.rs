use ide::{FilePosition, folding_ranges::FoldingConfig};
use itertools::Itertools;
use utils::text_edit::TextRange;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{from_proto, to_proto},
};

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
        let url = to_proto::url(&snap, file_id)?;
        symbols.into_iter().for_each(|symbol| {
            to_proto::document_symbol_information(symbol, url.clone(), &line_info, &mut res);
        });
        res.into()
    };

    Ok(Some(res))
}

pub(crate) fn handle_workspace_symbol(
    snap: GlobalStateSnapshot,
    params: lsp_types::WorkspaceSymbolParams,
) -> anyhow::Result<Option<lsp_types::WorkspaceSymbolResponse>> {
    let mut file_ids = snap.file_ids();
    file_ids.sort_unstable_by_key(|file_id| file_id.0);
    file_ids.dedup();

    let symbols = snap.analysis.workspace_symbol(&params.query, file_ids)?;
    let mut last_file_info = None;
    let res = symbols
        .into_iter()
        .map(|symbol| {
            let file_id = symbol.file_id;
            if last_file_info
                .as_ref()
                .is_none_or(|(cached_file_id, _, _)| *cached_file_id != file_id)
            {
                last_file_info =
                    Some((file_id, to_proto::url(&snap, file_id)?, snap.line_info(file_id)?));
            }
            let (_, url, line_info) = last_file_info.as_ref().unwrap();
            Ok(to_proto::workspace_symbol_information(symbol, url, line_info))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(Some(lsp_types::WorkspaceSymbolResponse::Flat(res)))
}

pub(crate) fn handle_selection_range(
    snap: GlobalStateSnapshot,
    params: lsp_types::SelectionRangeParams,
) -> anyhow::Result<Option<Vec<lsp_types::SelectionRange>>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let line_info = snap.line_info(file_id)?;

    let res = params
        .positions
        .into_iter()
        .map(|pos| {
            let offset = from_proto::offset(&line_info, pos)?;
            let ranges = snap.analysis.selection_ranges(FilePosition { file_id, offset })?;
            Ok(to_proto::selection_ranges(&line_info, ranges).unwrap_or_else(|| {
                lsp_types::SelectionRange {
                    range: to_proto::range(&line_info, TextRange::empty(offset)),
                    parent: None,
                }
            }))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(Some(res))
}

pub(crate) fn handle_folding_ranges(
    snap: GlobalStateSnapshot,
    params: lsp_types::FoldingRangeParams,
) -> anyhow::Result<Option<Vec<lsp_types::FoldingRange>>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let config = FoldingConfig { line_fold_only: snap.config.cli_line_folding_only() };
    let text = snap.file_text(file_id)?;
    let line_info = snap.line_info(file_id)?;

    let folds = snap
        .analysis
        .folding_ranges(file_id, &config)?
        .into_iter()
        .map(|fold| to_proto::folding_range(&text, &line_info, &config, fold))
        .collect();

    Ok(Some(folds))
}
