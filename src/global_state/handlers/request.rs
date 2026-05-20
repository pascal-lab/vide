use std::{
    collections::{HashMap, HashSet},
    fs,
};

use ide::{folding_ranges::FoldingConfig, references::References};
use itertools::Itertools;
use project_model::{
    TomlManifestField, toml_manifest_field_at_offset, toml_manifest_fields,
    toml_manifest_path_at_offset,
};
use span::{FilePosition, FileRange};
use utils::{
    paths::AbsPath,
    text_edit::{TextRange, TextSize},
};
use vfs::FileId;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{
        ext::{RUN_QIHE_ANALYSIS_COMMAND, RunQiheAnalysisParams},
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
    if snap.is_manifest_file(position.file_id) {
        return manifest_goto_definition(&snap, position);
    }

    let Some(nav_info) = snap.analysis.goto_definition(position)? else {
        return Ok(None);
    };

    let src = FileRange { file_id: position.file_id, range: nav_info.range };
    let res = to_proto::goto_definition_response(&snap, Some(src), nav_info.info)?;
    Ok(Some(res))
}

fn manifest_goto_definition(
    snap: &GlobalStateSnapshot,
    position: FilePosition,
) -> anyhow::Result<Option<lsp_types::GotoDefinitionResponse>> {
    let text = snap.file_text(position.file_id)?;
    let offset = usize::try_from(u32::from(position.offset)).unwrap_or(usize::MAX).min(text.len());
    let Some(context) = toml_manifest_path_at_offset(&text, offset) else {
        return Ok(None);
    };
    if context.value.is_empty() {
        return Ok(None);
    }

    let Some(manifest_path) = snap.file_abs_path(position.file_id) else {
        return Ok(None);
    };
    let Some(manifest_dir) = manifest_path.parent() else {
        return Ok(None);
    };
    let target = manifest_dir.absolutize(context.value.replace('\\', "/"));
    if fs::metadata(target.as_path()).is_err() {
        return Ok(None);
    }

    let uri = to_proto::url_from_abs_path(target.as_path())?;
    Ok(Some(lsp_types::GotoDefinitionResponse::Scalar(lsp_types::Location {
        uri,
        range: lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            lsp_types::Position::new(0, 0),
        ),
    })))
}

pub(crate) fn handle_completion(
    snap: GlobalStateSnapshot,
    params: lsp_types::CompletionParams,
) -> anyhow::Result<Option<lsp_types::CompletionResponse>> {
    use ide::completion::{CompletionItemKind as IdeCompletionItemKind, context::TriggerChar};
    use lsp_types::CompletionTextEdit;

    let position = from_proto::file_position(&snap, params.text_document_position)?;
    if snap.is_manifest_file(position.file_id) {
        return manifest_completion(&snap, position);
    }

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

fn manifest_completion(
    snap: &GlobalStateSnapshot,
    position: FilePosition,
) -> anyhow::Result<Option<lsp_types::CompletionResponse>> {
    let text = snap.file_text(position.file_id)?;
    let offset = usize::try_from(u32::from(position.offset)).unwrap_or(usize::MAX).min(text.len());
    let path_completion = manifest_path_completion(snap, position.file_id, &text, offset)?;
    if let Some(path_completion) = path_completion {
        return Ok(Some(path_completion));
    }

    let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_prefix = &text[line_start..offset];
    if line_prefix.trim_start().starts_with('#')
        || line_prefix.contains('=')
        || line_prefix.matches('"').count() % 2 == 1
    {
        return Ok(None);
    }

    let replace_start = line_start
        + line_prefix
            .char_indices()
            .rev()
            .find_map(|(idx, ch)| (!is_manifest_key_char(ch)).then_some(idx + ch.len_utf8()))
            .unwrap_or(0);
    let replacement = TextRange::new(to_text_size(replace_start), to_text_size(offset));
    let line_info = snap.line_info(position.file_id)?;
    let snippet_support = snap.config.cli_completion_snippet_support();

    let items = MANIFEST_FIELD_COMPLETIONS
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let new_text = if snippet_support { item.snippet } else { item.plain };
            lsp_types::CompletionItem {
                label: item.key.to_string(),
                kind: Some(lsp_types::CompletionItemKind::FIELD),
                detail: Some(item.detail.to_string()),
                documentation: Some(lsp_types::Documentation::String(
                    item.documentation.to_string(),
                )),
                sort_text: Some(format!("{idx:02}_{}", item.key)),
                insert_text_format: snippet_support.then_some(lsp_types::InsertTextFormat::SNIPPET),
                text_edit: Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
                    range: to_proto::range(&line_info, replacement),
                    new_text: new_text.to_string(),
                })),
                ..Default::default()
            }
        })
        .collect();

    Ok(Some(lsp_types::CompletionResponse::Array(items)))
}

fn manifest_path_completion(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
    text: &str,
    offset: usize,
) -> anyhow::Result<Option<lsp_types::CompletionResponse>> {
    let Some(context) = toml_manifest_path_at_offset(text, offset) else {
        return Ok(None);
    };
    let Some(manifest_path) = snap.file_abs_path(file_id) else {
        return Ok(None);
    };
    let Some(manifest_dir) = manifest_path.parent() else {
        return Ok(None);
    };

    let line_info = snap.line_info(file_id)?;
    let replacement =
        TextRange::new(to_text_size(context.content_range.start), to_text_size(offset));
    let prefix = &text[context.content_range.start..offset];
    let items = manifest_path_completion_items(
        manifest_dir,
        prefix,
        to_proto::range(&line_info, replacement),
    );

    Ok(Some(lsp_types::CompletionResponse::Array(items)))
}

fn manifest_path_completion_items(
    manifest_dir: &AbsPath,
    prefix: &str,
    replacement: lsp_types::Range,
) -> Vec<lsp_types::CompletionItem> {
    let prefix = prefix.replace('\\', "/");
    let (dir_prefix, name_prefix) = prefix
        .rsplit_once('/')
        .map(|(dir, name)| (format!("{dir}/"), name))
        .unwrap_or_else(|| (String::new(), prefix.as_str()));
    let search_dir = if dir_prefix.is_empty() {
        manifest_dir.to_path_buf()
    } else {
        manifest_dir.absolutize(&dir_prefix)
    };

    let Ok(entries) = fs::read_dir(search_dir.as_path()) else {
        return Vec::new();
    };

    let mut items = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with(name_prefix) {
                return None;
            }

            let file_type = entry.file_type().ok()?;
            let is_dir = file_type.is_dir();
            let completion_text =
                format!("{}{}{}", dir_prefix, name, if is_dir { "/" } else { "" });
            let kind = if is_dir {
                lsp_types::CompletionItemKind::FOLDER
            } else {
                lsp_types::CompletionItemKind::FILE
            };
            let sort_prefix = if is_dir { '0' } else { '1' };

            Some(lsp_types::CompletionItem {
                label: completion_text.clone(),
                kind: Some(kind),
                detail: Some(if is_dir { "Directory" } else { "File" }.to_string()),
                sort_text: Some(format!("{sort_prefix}_{completion_text}")),
                text_edit: Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
                    range: replacement,
                    new_text: completion_text,
                })),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|lhs, rhs| lhs.sort_text.cmp(&rhs.sort_text));
    items
}

fn manifest_hover(
    snap: &GlobalStateSnapshot,
    position: FilePosition,
) -> anyhow::Result<Option<lsp_types::Hover>> {
    let text = snap.file_text(position.file_id)?;
    let offset = usize::try_from(u32::from(position.offset)).unwrap_or(usize::MAX).min(text.len());
    let Some(field) = toml_manifest_field_at_offset(&text, offset) else {
        return Ok(None);
    };
    let Some(item) = MANIFEST_FIELD_COMPLETIONS.iter().find(|item| item.key == field.key) else {
        return Ok(None);
    };

    let line_info = snap.line_info(position.file_id)?;
    let value = format!("`{}`\n\n{}\n\n{}", item.key, item.detail, item.documentation);

    Ok(Some(lsp_types::Hover {
        contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value,
        }),
        range: Some(to_proto::range(
            &line_info,
            TextRange::new(to_text_size(field.key_range.start), to_text_size(field.key_range.end)),
        )),
    }))
}

fn is_manifest_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn to_text_size(value: usize) -> TextSize {
    TextSize::new(u32::try_from(value).unwrap_or(u32::MAX))
}

struct ManifestFieldCompletion {
    key: &'static str,
    plain: &'static str,
    snippet: &'static str,
    detail: &'static str,
    documentation: &'static str,
}

const MANIFEST_FIELD_COMPLETIONS: &[ManifestFieldCompletion] = &[
    ManifestFieldCompletion {
        key: "sources",
        plain: "sources = [\"rtl\"]",
        snippet: "sources = [\"${1:rtl}\"]",
        detail: "Source scan roots",
        documentation: "Directories or files to load as source roots. Omitted sources do not scan the workspace root.",
    },
    ManifestFieldCompletion {
        key: "include_dirs",
        plain: "include_dirs = [\"include\"]",
        snippet: "include_dirs = [\"${1:include}\"]",
        detail: "Include search roots",
        documentation: "Directories used for preprocessing include lookup. Omitted include_dirs default to the final sources.",
    },
    ManifestFieldCompletion {
        key: "defines",
        plain: "defines = [\"SYNTHESIS\"]",
        snippet: "defines = [\"${1:SYNTHESIS}\"]",
        detail: "Predefined macros",
        documentation: "Predefine macros as NAME or NAME=value strings.",
    },
    ManifestFieldCompletion {
        key: "libraries",
        plain: "libraries = [\"../lib\"]",
        snippet: "libraries = [\"${1:../lib}\"]",
        detail: "Library workspaces",
        documentation: "External library or dependency workspace paths.",
    },
    ManifestFieldCompletion {
        key: "top_modules",
        plain: "top_modules = [\"top\"]",
        snippet: "top_modules = [\"${1:top}\"]",
        detail: "Top modules",
        documentation: "Optional top module names for the compilation profile.",
    },
    ManifestFieldCompletion {
        key: "exclude",
        plain: "exclude = [\"build\"]",
        snippet: "exclude = [\"${1:build}\"]",
        detail: "Excluded paths",
        documentation: "Paths to remove from sources, include_dirs, and libraries.",
    },
];

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
    let result_id = snap.diagnostic_result_id(file_id);
    let items = snap.lsp_diagnostics(file_id);
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
    let mut diagnosed_roots = HashSet::new();

    for file_id in snap.file_ids() {
        let mut source_root_file_ids = snap.source_root_file_ids(file_id);
        source_root_file_ids.sort_unstable_by_key(|file_id| file_id.0);
        if !diagnosed_roots.insert(source_root_file_ids) {
            continue;
        }

        for diag in snap.source_root_diagnostics(file_id)? {
            diagnostics_by_file.entry(diag.file_id).or_insert_with(Vec::new).push(diag);
        }
    }

    for file_id in snap.file_ids() {
        let uri = match to_proto::url(&snap, file_id) {
            Ok(uri) => uri,
            Err(error) => {
                tracing::debug!(?file_id, "skipping diagnostics for file without URI: {error:#}");
                continue;
            }
        };
        seen.insert(uri.clone());

        let diagnostics = diagnostics_by_file.remove(&file_id).unwrap_or_default();

        let line_info = snap.line_info(file_id)?;
        let mut diag_items = diagnostics
            .into_iter()
            .map(|diag| to_proto::diagnostic(&line_info, diag))
            .collect::<Vec<_>>();
        diag_items.extend(snap.manifest_lsp_diagnostics(file_id));
        diag_items.extend(snap.qihe_diagnostics(file_id));

        let result_id = snap.diagnostic_result_id(file_id);
        let version = snap.file_version(file_id).map(|version| version as i64);
        let previous_result_id = previous_result_ids.get(&uri).map(String::as_str);

        items.push(workspace_diagnostic_report(
            uri,
            version,
            result_id,
            diag_items,
            previous_result_id,
        ));
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
    let args = params
        .arguments
        .first()
        .cloned()
        .ok_or_else(|| anyhow::format_err!("missing executeCommand arguments"))?;
    let params = serde_json::from_value::<RunQiheAnalysisParams>(args)?;
    state.spawn_qihe_analysis(params);
    Ok(None)
}

pub(crate) fn handle_execute_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    match params.command.as_str() {
        RUN_QIHE_ANALYSIS_COMMAND => handle_qihe_analysis_command(state, params),
        _ => anyhow::bail!("unknown executeCommand: {}", params.command),
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
    if snap.is_manifest_file(file_id) {
        return manifest_document_symbols(&snap, file_id);
    }

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

fn manifest_document_symbols(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
) -> anyhow::Result<Option<lsp_types::DocumentSymbolResponse>> {
    let text = snap.file_text(file_id)?;
    let line_info = snap.line_info(file_id)?;
    let fields = toml_manifest_fields(&text);

    let res = if snap.config.hierarchical_symbols() {
        fields
            .into_iter()
            .map(|field| manifest_document_symbol(&line_info, field))
            .collect_vec()
            .into()
    } else {
        let url = to_proto::url(snap, file_id)?;
        fields
            .into_iter()
            .map(|field| manifest_symbol_information(&line_info, url.clone(), field))
            .collect_vec()
            .into()
    };

    Ok(Some(res))
}

#[allow(deprecated)]
fn manifest_document_symbol(
    line_info: &utils::lines::LineInfo,
    field: TomlManifestField,
) -> lsp_types::DocumentSymbol {
    let range =
        TextRange::new(to_text_size(field.key_range.start), to_text_size(field.value_range.end));
    let selection_range =
        TextRange::new(to_text_size(field.key_range.start), to_text_size(field.key_range.end));

    lsp_types::DocumentSymbol {
        name: field.key,
        detail: None,
        kind: lsp_types::SymbolKind::PROPERTY,
        tags: None,
        deprecated: None,
        range: to_proto::range(line_info, range),
        selection_range: to_proto::range(line_info, selection_range),
        children: None,
    }
}

#[allow(deprecated)]
fn manifest_symbol_information(
    line_info: &utils::lines::LineInfo,
    uri: lsp_types::Url,
    field: TomlManifestField,
) -> lsp_types::SymbolInformation {
    let range =
        TextRange::new(to_text_size(field.key_range.start), to_text_size(field.value_range.end));

    lsp_types::SymbolInformation {
        name: field.key,
        kind: lsp_types::SymbolKind::PROPERTY,
        tags: None,
        deprecated: None,
        location: lsp_types::Location { uri, range: to_proto::range(line_info, range) },
        container_name: None,
    }
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
    if snap.is_manifest_file(position.file_id) {
        return manifest_hover(&snap, position);
    }

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
