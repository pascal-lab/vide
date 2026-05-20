use std::fs;

use itertools::Itertools;
use project_model::{
    TomlManifestField, toml_manifest_field_at_offset, toml_manifest_field_completion_context,
    toml_manifest_fields, toml_manifest_path_at_offset, toml_manifest_paths,
};
use span::FilePosition;
use utils::{
    lines::LineInfo,
    paths::{AbsPath, AbsPathBuf},
    text_edit::{TextRange, TextSize},
};
use vfs::FileId;

use crate::{global_state::snapshot::GlobalStateSnapshot, lsp_ext::to_proto};

pub(super) fn goto_definition(
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

    let Some(target) = path_target(snap, position.file_id, &context.value) else {
        return Ok(None);
    };

    let uri = to_proto::url_from_abs_path(target.as_path())?;
    Ok(Some(lsp_types::GotoDefinitionResponse::Scalar(lsp_types::Location {
        uri,
        range: lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            lsp_types::Position::new(0, 0),
        ),
    })))
}

pub(super) fn completion(
    snap: &GlobalStateSnapshot,
    position: FilePosition,
) -> anyhow::Result<Option<lsp_types::CompletionResponse>> {
    let text = snap.file_text(position.file_id)?;
    let offset = usize::try_from(u32::from(position.offset)).unwrap_or(usize::MAX).min(text.len());
    let path_completion = path_completion(snap, position.file_id, &text, offset)?;
    if let Some(path_completion) = path_completion {
        return Ok(Some(path_completion));
    }

    let Some(context) = toml_manifest_field_completion_context(&text, offset) else {
        return Ok(None);
    };
    let replacement = TextRange::new(
        to_text_size(context.replacement_range.start),
        to_text_size(context.replacement_range.end),
    );
    let line_info = snap.line_info(position.file_id)?;
    let snippet_support = snap.config.cli_completion_snippet_support();

    let items = MANIFEST_FIELD_COMPLETIONS
        .iter()
        .enumerate()
        .filter(|(_, item)| !context.existing_fields.iter().any(|field| field == item.key))
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

pub(super) fn hover(
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

pub(super) fn document_link(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
) -> anyhow::Result<Option<Vec<lsp_types::DocumentLink>>> {
    let text = snap.file_text(file_id)?;
    let line_info = snap.line_info(file_id)?;
    let links = toml_manifest_paths(&text)
        .into_iter()
        .filter_map(|path| {
            let target = path_target(snap, file_id, &path.value)?;
            let range = TextRange::new(
                to_text_size(path.content_range.start),
                to_text_size(path.content_range.end),
            );
            Some(lsp_types::DocumentLink {
                range: to_proto::range(&line_info, range),
                target: to_proto::url_from_abs_path(target.as_path()).ok(),
                tooltip: Some(format!("Open {}", path.value)),
                data: None,
            })
        })
        .collect();

    Ok(Some(links))
}

pub(super) fn document_symbols(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
) -> anyhow::Result<Option<lsp_types::DocumentSymbolResponse>> {
    let text = snap.file_text(file_id)?;
    let line_info = snap.line_info(file_id)?;
    let fields = toml_manifest_fields(&text);

    let res = if snap.config.hierarchical_symbols() {
        fields.into_iter().map(|field| document_symbol(&line_info, field)).collect_vec().into()
    } else {
        let url = to_proto::url(snap, file_id)?;
        fields
            .into_iter()
            .map(|field| symbol_information(&line_info, url.clone(), field))
            .collect_vec()
            .into()
    };

    Ok(Some(res))
}

fn path_completion(
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
    let items =
        path_completion_items(manifest_dir, prefix, to_proto::range(&line_info, replacement));

    Ok(Some(lsp_types::CompletionResponse::Array(items)))
}

fn path_completion_items(
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

fn path_target(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
    path_value: &str,
) -> Option<AbsPathBuf> {
    let manifest_path = snap.file_abs_path(file_id)?;
    let manifest_dir = manifest_path.parent()?;
    let target = manifest_dir.absolutize(path_value.replace('\\', "/"));
    fs::metadata(target.as_path()).ok()?;
    Some(target)
}

#[allow(deprecated)]
fn document_symbol(line_info: &LineInfo, field: TomlManifestField) -> lsp_types::DocumentSymbol {
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
fn symbol_information(
    line_info: &LineInfo,
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
