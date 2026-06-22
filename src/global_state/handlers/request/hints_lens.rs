use ide::FileRange;
use itertools::Itertools;
use utils::text_edit::TextRange;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp::protocol::{from_proto, to_proto},
};

pub(crate) fn handle_inlay_hint(
    snap: GlobalStateSnapshot,
    params: lsp_types::InlayHintParams,
) -> anyhow::Result<Option<Vec<lsp_types::InlayHint>>> {
    let FileRange { file_id, range } =
        from_proto::file_range(&snap, &params.text_document.uri, params.range)?;

    let line_info = snap.line_info(file_id)?;
    let range = TextRange::new(
        range.start().min(line_info.index.text_len()),
        range.end().min(line_info.index.text_len()),
    );

    let config = snap.config.inlay_hint();
    let res = snap
        .analysis
        .inlay_hint(file_id, range, config)?
        .into_iter()
        .map(|hint| to_proto::inlay_hint(&snap, &line_info, hint))
        .collect_vec();

    Ok(Some(res))
}

pub(crate) fn handle_code_lens(
    snap: GlobalStateSnapshot,
    params: lsp_types::CodeLensParams,
) -> anyhow::Result<Option<Vec<lsp_types::CodeLens>>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let line_info = snap.line_info(file_id)?;
    let config = snap.config.code_lens();

    let res = snap
        .analysis
        .code_lens(file_id, config)?
        .into_iter()
        .filter_map(|lens| to_proto::code_lens(&snap, &line_info, file_id, lens))
        .collect();

    Ok(Some(res))
}

pub(crate) fn handle_code_lens_resolve(
    snap: GlobalStateSnapshot,
    mut code_lens: lsp_types::CodeLens,
) -> anyhow::Result<lsp_types::CodeLens> {
    let Some(data) = code_lens.data.take() else {
        return Ok(code_lens);
    };

    let (file_id, code_lens_kind) = from_proto::code_lens(&snap, data)?;
    let code_lens_kind = snap.analysis.code_lens_resolve(code_lens_kind)?;

    let line_info = snap.line_info(file_id)?;
    let (command, data) = to_proto::code_lens_kind(&snap, file_id, &line_info, code_lens_kind)?;
    let res = lsp_types::CodeLens { range: code_lens.range, command, data };

    Ok(res)
}

pub(crate) fn handle_signature_help(
    snap: GlobalStateSnapshot,
    params: lsp_types::SignatureHelpParams,
) -> anyhow::Result<Option<lsp_types::SignatureHelp>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let config = snap.config.signature_help();
    let Some(res) = snap.analysis.signature_help(position, config)? else {
        return Ok(None);
    };

    let support_label_offsets = snap.config.cli_signature_help_label_offsets_support();
    let res = to_proto::signature_help(res, support_label_offsets);
    Ok(Some(res))
}
