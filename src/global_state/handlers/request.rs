use std::collections::{HashMap, HashSet};

use ide::{folding_ranges::FoldingConfig, references::References};
use itertools::Itertools;
use span::{FilePosition, FileRange};
use utils::text_edit::TextRange;
use vfs::FileId;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    i18n::keys,
    lsp_ext::{
        ext::{RELOAD_WORKSPACE_COMMAND, RUN_QIHE_ANALYSIS_COMMAND, RunQiheAnalysisParams},
        from_proto, to_proto,
    },
};

mod code_action;
pub(crate) use code_action::{handle_code_action, handle_code_action_resolve};

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
    use ide::completion::{CompletionItemKind as IdeCompletionItemKind, context::TriggerChar};
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
            '@' => Some(TriggerChar::At),
            '#' => Some(TriggerChar::Hash),
            '$' => Some(TriggerChar::Dollar),
            '`' => Some(TriggerChar::Backtick),
            '\'' => Some(TriggerChar::Apostrophe),
            '\n' | '\r' => Some(TriggerChar::Newline),
            _ => None,
        });

    let snippet_support = snap.config.cli_completion_snippet_support();
    let items = snap.analysis.completions_with_trigger(position, trigger)?;
    let items = items
        .into_iter()
        .filter_map(|item| {
            let sort_text = item.sort_text();
            let (edit, insert_text_format) = if snippet_support {
                match (item.snippet_edit, item.edit) {
                    (Some(edit), _) => Some((edit, Some(lsp_types::InsertTextFormat::SNIPPET))),
                    (None, Some(edit)) => Some((edit, None)),
                    (None, None) => None,
                }
            } else {
                item.edit.map(|edit| (edit, None))
            }?;

            let kind = match item.kind {
                IdeCompletionItemKind::Text => lsp_types::CompletionItemKind::TEXT,
                IdeCompletionItemKind::Keyword => lsp_types::CompletionItemKind::KEYWORD,
                IdeCompletionItemKind::Snippet => lsp_types::CompletionItemKind::SNIPPET,
            };

            Some(lsp_types::CompletionItem {
                label: item.label,
                kind: Some(kind),
                sort_text: Some(sort_text),
                insert_text_format,
                text_edit: Some(CompletionTextEdit::Edit(to_proto::text_edit(&line_info, edit))),
                ..Default::default()
            })
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

pub(crate) fn handle_document_diagnostic(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentDiagnosticParams,
) -> anyhow::Result<lsp_types::DocumentDiagnosticReportResult> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let result_id = snap.document_diagnostic_result_id(file_id, &params.text_document.uri);
    let items = snap.lsp_diagnostics(file_id)?;
    Ok(document_diagnostic_report(result_id, items, params.previous_result_id.as_deref()).into())
}

pub(crate) fn handle_workspace_diagnostic(
    snap: GlobalStateSnapshot,
    params: lsp_types::WorkspaceDiagnosticParams,
) -> anyhow::Result<lsp_types::WorkspaceDiagnosticReportResult> {
    let previous_result_ids = params
        .previous_result_ids
        .into_iter()
        .map(|prev| {
            let original_uri = prev.uri;
            let uri = match from_proto::abs_path(&original_uri)
                .and_then(|path| to_proto::url_from_abs_path(path.as_ref()))
            {
                Ok(uri) => uri,
                Err(error) => {
                    tracing::debug!(
                        uri = %original_uri,
                        "keeping previous diagnostic URI as-is: {error:#}"
                    );
                    original_uri
                }
            };
            (uri, prev.value)
        })
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut diagnostics_by_file = HashMap::new();

    let diagnostic_file_ids = snap.workspace_diagnostic_file_ids();

    for producer in snap.workspace_diagnostic_producers(&diagnostic_file_ids) {
        for diag in snap.workspace_diagnostics_for_producer(&producer)? {
            diagnostics_by_file.entry(diag.file_id).or_insert_with(Vec::new).push(diag);
        }
    }

    for file_id in diagnostic_file_ids {
        let targets = match snap.workspace_diagnostic_targets(file_id) {
            Ok(targets) => targets,
            Err(error) => {
                tracing::debug!(?file_id, "skipping diagnostics for file without URI: {error:#}");
                continue;
            }
        };

        let diagnostics = diagnostics_by_file.remove(&file_id).unwrap_or_default();

        let line_info = snap.line_info(file_id)?;
        let mut diag_items = diagnostics
            .into_iter()
            .map(|diag| to_proto::diagnostic(snap.config.i18n, &line_info, diag))
            .collect::<Vec<_>>();
        diag_items.extend(snap.qihe_diagnostics(file_id));

        for target in targets {
            let uri = target.uri().clone();
            seen.insert(uri.clone());
            let result_id = snap.workspace_diagnostic_result_id(file_id, &uri);
            let version = target.version().map(|version| version as i64);
            let previous_result_id = previous_result_ids.get(&uri).map(String::as_str);

            items.push(workspace_diagnostic_report(
                uri,
                version,
                result_id,
                diag_items.clone(),
                previous_result_id,
            ));
        }
    }

    for (uri, _) in previous_result_ids {
        if seen.contains(&uri) {
            continue;
        }

        items.push(workspace_diagnostic_report(uri, None, None, Vec::new(), None));
    }

    Ok(lsp_types::WorkspaceDiagnosticReportResult::Report(lsp_types::WorkspaceDiagnosticReport {
        items,
    }))
}

fn document_diagnostic_report(
    result_id: Option<String>,
    items: Vec<lsp_types::Diagnostic>,
    previous_result_id: Option<&str>,
) -> lsp_types::DocumentDiagnosticReport {
    if let Some(result_id) = result_id.as_ref()
        && Some(result_id.as_str()) == previous_result_id
    {
        return lsp_types::DocumentDiagnosticReport::Unchanged(
            lsp_types::RelatedUnchangedDocumentDiagnosticReport {
                related_documents: None,
                unchanged_document_diagnostic_report:
                    lsp_types::UnchangedDocumentDiagnosticReport { result_id: result_id.clone() },
            },
        );
    }

    lsp_types::DocumentDiagnosticReport::Full(lsp_types::RelatedFullDocumentDiagnosticReport {
        related_documents: None,
        full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
            result_id: result_id.clone(),
            items,
        },
    })
}

fn handle_qihe_analysis_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    let args = params.arguments.first().cloned().ok_or_else(|| {
        anyhow::format_err!("{}", state.config.i18n.text(keys::EXECUTE_COMMAND_MISSING_ARGUMENTS))
    })?;
    let params = serde_json::from_value::<RunQiheAnalysisParams>(args)?;
    state.spawn_qihe_analysis(params);
    Ok(None)
}

fn handle_reload_workspace_command(
    state: &mut crate::global_state::GlobalState,
) -> anyhow::Result<Option<serde_json::Value>> {
    let config = triomphe::Arc::make_mut(&mut state.config);
    config.refresh_project_manifests();
    state.request_workspace_reload("workspace reload command");
    Ok(None)
}

pub(crate) fn handle_execute_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    match params.command.as_str() {
        RUN_QIHE_ANALYSIS_COMMAND => handle_qihe_analysis_command(state, params),
        RELOAD_WORKSPACE_COMMAND => handle_reload_workspace_command(state),
        _ => anyhow::bail!(
            "{}",
            state
                .config
                .i18n
                .format(keys::EXECUTE_COMMAND_UNKNOWN, [("command", params.command.clone())])
        ),
    }
}

fn workspace_diagnostic_report(
    uri: lsp_types::Url,
    version: Option<i64>,
    result_id: Option<String>,
    items: Vec<lsp_types::Diagnostic>,
    previous_result_id: Option<&str>,
) -> lsp_types::WorkspaceDocumentDiagnosticReport {
    if let Some(result_id) = result_id.as_ref()
        && Some(result_id.as_str()) == previous_result_id
    {
        return lsp_types::WorkspaceDocumentDiagnosticReport::Unchanged(
            lsp_types::WorkspaceUnchangedDocumentDiagnosticReport {
                uri,
                version,
                unchanged_document_diagnostic_report:
                    lsp_types::UnchangedDocumentDiagnosticReport { result_id: result_id.clone() },
            },
        );
    }

    lsp_types::WorkspaceDocumentDiagnosticReport::Full(
        lsp_types::WorkspaceFullDocumentDiagnosticReport {
            uri,
            version,
            full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
                result_id: result_id.clone(),
                items,
            },
        },
    )
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
        let url = to_proto::url(&snap, file_id)?;
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
    if !snap.file_allows_workspace_edits(position.file_id) {
        return Ok(None);
    }
    let line_index = snap.line_info(position.file_id)?;

    let text_range = snap
        .analysis
        .prepare_rename(position)?
        .map_err(|err| to_proto::rename_error(snap.config.i18n, err))?;
    let range = to_proto::range(&line_index, text_range);
    Ok(Some(lsp_types::PrepareRenameResponse::Range(range)))
}

pub(crate) fn handle_rename(
    snap: GlobalStateSnapshot,
    params: lsp_types::RenameParams,
) -> anyhow::Result<Option<lsp_types::WorkspaceEdit>> {
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    if !snap.file_allows_workspace_edits(position.file_id) {
        return Ok(None);
    }
    let config = snap.config.rename();
    let change = snap
        .analysis
        .rename(position, config, &params.new_name)?
        .map_err(|err| to_proto::rename_error(snap.config.i18n, err))?;

    let workspace_edit = to_proto::workspace_edit(&snap, change)?;
    Ok(Some(workspace_edit))
}

pub(crate) fn handle_formatting(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentFormattingParams,
) -> anyhow::Result<Option<Vec<lsp_types::TextEdit>>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let line_info = snap.line_info(file_id)?;

    let config = formatting_config(&snap, &params.options);
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

    let config = formatting_config(&snap, &params.options);
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

    let config = formatting_config(&snap, &params.options);
    let edit = snap
        .analysis
        .format_on_type(position, params.ch, &line_info, config)?
        .map_err(to_proto::format_error)?;

    let text_edits = edit.map(|edit| to_proto::text_edits(&line_info, edit));
    Ok(text_edits)
}

fn formatting_config(
    snap: &GlobalStateSnapshot,
    options: &lsp_types::FormattingOptions,
) -> ide::formatting::FmtConfig {
    let mut config = snap.config.fmt();
    config.apply_editor_options(options.tab_size, options.insert_spaces);
    config
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

#[cfg(test)]
mod tests {
    use lsp_types::{
        DocumentDiagnosticReport, UnchangedDocumentDiagnosticReport, Url,
        WorkspaceDocumentDiagnosticReport,
    };

    use super::{document_diagnostic_report, workspace_diagnostic_report};

    #[test]
    fn workspace_diagnostic_report_uses_full_for_new_result_id() {
        let uri = Url::parse("file:///tmp/test.sv").unwrap();
        let report = workspace_diagnostic_report(
            uri.clone(),
            Some(3),
            Some("4".to_string()),
            Vec::new(),
            Some("2"),
        );

        match report {
            WorkspaceDocumentDiagnosticReport::Full(report) => {
                assert_eq!(report.uri, uri);
                assert_eq!(report.version, Some(3));
                assert_eq!(report.full_document_diagnostic_report.result_id.as_deref(), Some("4"));
                assert!(report.full_document_diagnostic_report.items.is_empty());
            }
            other => panic!("expected full report, got {other:?}"),
        }
    }

    #[test]
    fn workspace_diagnostic_report_uses_unchanged_for_matching_result_id() {
        let uri = Url::parse("file:///tmp/test.sv").unwrap();
        let report = workspace_diagnostic_report(
            uri.clone(),
            Some(5),
            Some("5".to_string()),
            Vec::new(),
            Some("5"),
        );

        match report {
            WorkspaceDocumentDiagnosticReport::Unchanged(report) => {
                assert_eq!(report.uri, uri);
                assert_eq!(report.version, Some(5));
                assert_eq!(report.unchanged_document_diagnostic_report.result_id, "5");
            }
            other => panic!("expected unchanged report, got {other:?}"),
        }
    }
    #[test]
    fn document_diagnostic_report_uses_unchanged_for_matching_result_id() {
        let report = document_diagnostic_report(Some("7".to_string()), Vec::new(), Some("7"));

        match report {
            DocumentDiagnosticReport::Unchanged(report) => assert_eq!(
                report.unchanged_document_diagnostic_report,
                UnchangedDocumentDiagnosticReport { result_id: "7".to_string() }
            ),
            other => panic!("expected unchanged report, got {other:?}"),
        }
    }
}
