use ide::{
    code_action::CodeActionResolveStrategy, folding_ranges::FoldingConfig, references::References,
};
use itertools::Itertools;
use span::{FilePosition, FileRange};
use utils::text_edit::TextRange;
use vfs::FileId;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{ext::CodeActionResolveError, from_proto, to_proto},
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

pub(crate) fn handle_completion(
    snap: GlobalStateSnapshot,
    params: lsp_types::CompletionParams,
) -> anyhow::Result<Option<lsp_types::CompletionResponse>> {
    use ide::completion::context::TriggerChar;
    use lsp_types::CompletionTextEdit;

    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let line_info = snap.line_info(position.file_id)?;

    let trigger = params
        .context
        .as_ref()
        .and_then(|ctx| ctx.trigger_character.as_deref())
        .and_then(|s| s.chars().next())
        .and_then(|ch| match ch {
            '.' => Some(TriggerChar::Dot),
            '(' => Some(TriggerChar::OpenParen),
            ',' => Some(TriggerChar::Comma),
            ' ' => Some(TriggerChar::Space),
            '@' => Some(TriggerChar::At),
            '#' => Some(TriggerChar::Hash),
            '`' => Some(TriggerChar::Backtick),
            _ => None,
        });

    let items = snap.analysis.completions_with_trigger(position, trigger)?;
    let items = items
        .into_iter()
        .map(|item| lsp_types::CompletionItem {
            label: item.label,
            kind: Some(lsp_types::CompletionItemKind::TEXT),
            text_edit: item
                .edit
                .map(|edit| CompletionTextEdit::Edit(to_proto::text_edit(&line_info, edit))),
            ..Default::default()
        })
        .collect();

    Ok(Some(lsp_types::CompletionResponse::Array(items)))
}

pub(crate) fn handle_goto_declaration(
    snap: GlobalStateSnapshot,
    params: lsp_types::request::GotoDeclarationParams,
) -> anyhow::Result<Option<lsp_types::request::GotoDeclarationResponse>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params.clone())?;
    let Some(nav_info) = snap.analysis.goto_declaration(position)? else {
        return handle_goto_definition(snap, params);
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
    let config = snap.config.document_highlight();
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
    let config = snap.config.references();
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
) -> anyhow::Result<Option<lsp_types::PrepareRenameResponse>> {
    let position = from_proto::file_position(&snap, params)?;
    let line_index = snap.line_info(position.file_id)?;

    let text_range = snap.analysis.prepare_rename(position)?.map_err(to_proto::rename_error)?;
    let range = to_proto::range(&line_index, text_range);
    Ok(Some(lsp_types::PrepareRenameResponse::Range(range)))
}

pub(crate) fn handle_rename(
    snap: GlobalStateSnapshot,
    params: lsp_types::RenameParams,
) -> anyhow::Result<Option<lsp_types::WorkspaceEdit>> {
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.config.rename();
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

    let config = snap.config.fmt();
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
    let line_ranges =
        Some((params.range.start.line as usize)..((params.range.end.line as usize) + 1));

    let config = snap.config.fmt();
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

    let config = snap.config.fmt();
    let edit = snap
        .analysis
        .format_on_type(position, params.ch, &line_info, config)?
        .map_err(to_proto::format_error)?;

    let text_edits = edit.map(|edit| to_proto::text_edits(&line_info, edit));
    Ok(text_edits)
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
            Ok(to_proto::selection_ranges(&line_info, ranges))
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

pub(crate) fn handle_hover(
    snap: GlobalStateSnapshot,
    params: lsp_types::HoverParams,
) -> anyhow::Result<Option<lsp_types::Hover>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;

    let config = snap.config.hover();
    let hover_format = config.format;
    let Some(hover_info) = snap.analysis.hover(position, config)? else {
        return Ok(None);
    };

    let line_info = snap.line_info(position.file_id)?;
    let range = to_proto::range(&line_info, hover_info.range);

    let res = lsp_types::Hover {
        contents: to_proto::hover_contents(hover_info.info, hover_format),
        range: Some(range),
    };

    Ok(Some(res))
}

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

pub(crate) fn handle_semantic_tokens_full(
    snap: GlobalStateSnapshot,
    params: lsp_types::SemanticTokensParams,
) -> anyhow::Result<Option<lsp_types::SemanticTokensResult>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let res = compute_sema_tokens_helper(&snap, file_id, None)?;
    snap.sema_tokens_cache.lock().insert(params.text_document.uri, res.clone());
    Ok(Some(res.into()))
}

pub(crate) fn handle_semantic_tokens_full_delta(
    snap: GlobalStateSnapshot,
    params: lsp_types::SemanticTokensDeltaParams,
) -> anyhow::Result<Option<lsp_types::SemanticTokensFullDeltaResult>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let res = compute_sema_tokens_helper(&snap, file_id, None)?;

    let old_tokens = snap.sema_tokens_cache.lock().remove(&params.text_document.uri);
    if let Some(old_tokens @ lsp_types::SemanticTokens { result_id: Some(prev_id), .. }) =
        &old_tokens
        && *prev_id == params.previous_result_id
    {
        let delta = to_proto::semantic_token_delta(old_tokens, &res);
        snap.sema_tokens_cache.lock().insert(params.text_document.uri, res);
        Ok(Some(delta.into()))
    } else {
        // Clone first to keep the lock short
        let semantic_tokens_clone = res.clone();
        snap.sema_tokens_cache.lock().insert(params.text_document.uri, semantic_tokens_clone);
        Ok(Some(res.into()))
    }
}

pub(crate) fn handle_semantic_tokens_range(
    snap: GlobalStateSnapshot,
    params: lsp_types::SemanticTokensRangeParams,
) -> anyhow::Result<Option<lsp_types::SemanticTokensRangeResult>> {
    let FileRange { file_id, range } =
        from_proto::file_range(&snap, &params.text_document.uri, params.range)?;
    let res = compute_sema_tokens_helper(&snap, file_id, Some(range))?;
    Ok(Some(res.into()))
}

fn compute_sema_tokens_helper(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
    range: Option<TextRange>,
) -> anyhow::Result<lsp_types::SemanticTokens> {
    let text = snap.analysis.file_text(file_id)?;
    let line_info = snap.line_info(file_id)?;
    let config = snap.config.semantic_tokens();
    let tokens = snap.analysis.semantic_tokens(file_id, config, range)?;

    let res = to_proto::semantic_tokens(&text, &line_info, tokens);
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

pub(crate) fn handle_code_action(
    snap: GlobalStateSnapshot,
    params: lsp_types::CodeActionParams,
) -> anyhow::Result<Option<Vec<lsp_types::CodeActionOrCommand>>> {
    if !snap.config.cli_code_action_literals() {
        return Ok(None);
    }

    let FileRange { file_id, range } =
        from_proto::file_range(&snap, &params.text_document.uri, params.range)?;

    let resolve_strategy = if snap.config.cli_code_action_resolve() {
        CodeActionResolveStrategy::None
    } else {
        CodeActionResolveStrategy::All
    };

    let action = snap.analysis.code_action(file_id, range, resolve_strategy.clone())?;

    let mut res = Vec::new();
    for (id, assist) in action.into_iter().enumerate() {
        let resolve_data =
            resolve_strategy.is_none().then(|| (id, params.clone(), snap.file_version(file_id)));
        let code_action = to_proto::code_action(&snap, assist, resolve_data)?;
        res.push(lsp_types::CodeActionOrCommand::CodeAction(code_action))
    }

    Ok(Some(res))
}

pub(crate) fn handle_code_action_resolve(
    snap: GlobalStateSnapshot,
    mut code_action: lsp_types::CodeAction,
) -> anyhow::Result<lsp_types::CodeAction> {
    fn parse_action_id(action_id: &str) -> anyhow::Result<(usize, String), String> {
        let id_parts = action_id.split(':').collect::<Vec<_>>();
        match id_parts.as_slice() {
            [assist_name, index] => {
                let index: usize = index.parse().map_err(|_| "Incorrect index string")?;
                Ok((index, assist_name.to_string()))
            }
            _ => Err("Action id contains incorrect number of segments".to_owned()),
        }
    }

    let data = from_proto::code_action_data(
        code_action.data.replace(Default::default()).ok_or(CodeActionResolveError::NoData)?,
    )?;

    let file_id = from_proto::file_id(&snap, &data.code_action_params.text_document.uri)?;
    if snap.file_version(file_id) != data.version {
        return Err(CodeActionResolveError::Stable.into());
    }

    let line_index = snap.line_info(file_id)?;
    let range = from_proto::text_range(&line_index, data.code_action_params.range)?;

    let (idx, name) = parse_action_id(&data.id).map_err(CodeActionResolveError::InvalidId)?;
    let resolve_strategy = CodeActionResolveStrategy::Single { name };

    let action = snap.analysis.code_action(file_id, range, resolve_strategy)?.remove(idx);

    let resolved_action = to_proto::code_action(&snap, action, None)?;
    code_action.edit = resolved_action.edit;
    code_action.command = resolved_action.command;

    Ok(code_action)
}
