//! IDE support for the `vide.toml` project manifest.
//!
//! Project manifests are deliberately not SystemVerilog parse units.  They
//! still need source-aware editor features, though, so this module owns the
//! small TOML model used by those features.  `toml_edit` is used instead of a
//! deserialized config because its immutable document keeps byte spans for
//! keys and values, including unsaved editor content.

use std::{collections::BTreeSet, ops::Range};

use base_db::source_db::{SourceDb, SourceFileKind};
use syntax::DiagnosticSeverity;
use toml_edit::{DocumentMut, ImDocument, Item, Value};
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    text_edit::{TextEdit, TextEditItem},
};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo, SymbolKind,
    completion::{CompletionItem, CompletionItemKind},
    db::{SourceFileQueryKey, root_db::RootDb, workspace_symbol_index_db::WorkspaceSymbolIndexDb},
    diagnostics::{Diagnostic, DiagnosticSource},
    document_highlight::DocumentHighlight,
    document_symbols::DocumentSymbol,
    folding_ranges::{Fold, FoldKind},
    markup::Markup,
    navigation_target::NavTarget,
    references::{ReferenceCategory, References, ReferencesConfig, ReferencesStatus},
    semantic_tokens::{SemaToken, SemaTokenModifier, SemaTokenTag},
    source_change::{SourceChange, SourceChangeBuilder},
};

const MANIFEST_KEYS: &[(&str, &str)] = &[
    ("top_modules", "Top-level module names used by the compilation profile."),
    ("defines", "Predefined macros. Use NAME or NAME=value strings."),
    ("sources", "Workspace-relative source file glob patterns."),
    ("include_dirs", "Include/search directories relative to the workspace root."),
    ("libraries", "External library or dependency workspace paths."),
    ("exclude", "Workspace-relative glob patterns removed from the loaded files."),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestValue {
    text: String,
    range: TextRange,
    content_range: TextRange,
    kind: ManifestValueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestValueKind {
    String,
    Integer,
    Boolean,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestEntry {
    key: String,
    key_range: TextRange,
    value_range: TextRange,
    full_range: TextRange,
    values: Vec<ManifestValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestIndex {
    entries: Vec<ManifestEntry>,
    error: Option<ManifestParseError>,
    formatted_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestParseError {
    range: Option<TextRange>,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManifestTarget {
    pub(crate) file_id: FileId,
    entry_index: usize,
    value_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct ManifestTargetInfo {
    pub(crate) file_id: FileId,
    pub(crate) key: String,
    pub(crate) key_range: TextRange,
    pub(crate) selected_value: Option<ManifestValue>,
    pub(crate) values: Vec<ManifestValue>,
}

fn is_manifest(db: &dyn SourceDb, file_id: FileId) -> bool {
    matches!(db.file_kind(file_id), SourceFileKind::ProjectManifest)
}

fn text_range(range: Range<usize>) -> Option<TextRange> {
    Some(TextRange::new(
        TextSize::from(u32::try_from(range.start).ok()?),
        TextSize::from(u32::try_from(range.end).ok()?),
    ))
}

fn parse_document(text: &str) -> Result<(Vec<ManifestEntry>, DocumentMut), toml_edit::TomlError> {
    let document = ImDocument::parse(text.to_owned())?;
    let editable_document = document.clone().into_mut();
    let mut entries = Vec::new();

    for (key, item) in document.iter() {
        let Some(value_range) = item.span().and_then(text_range) else {
            continue;
        };
        let key_range = key_range(text, value_range.start())
            .unwrap_or_else(|| TextRange::empty(value_range.start()));
        let full_range = TextRange::new(key_range.start(), value_range.end());
        let values = item_values(text, item);
        entries.push(ManifestEntry {
            key: key.to_owned(),
            key_range,
            value_range,
            full_range,
            values,
        });
    }

    entries.sort_by_key(|entry| entry.full_range.start());
    Ok((entries, editable_document))
}

fn parse(text: &str) -> Result<Vec<ManifestEntry>, toml_edit::TomlError> {
    parse_document(text).map(|(entries, _)| entries)
}

fn format_document(document: &mut DocumentMut) {
    format_table(document.as_table_mut());
}

fn format_table(table: &mut toml_edit::Table) {
    for (mut key, item) in table.iter_mut() {
        // Keep the key prefix intact: it contains comments and the line break
        // separating this entry from the previous one. Only normalize the
        // whitespace immediately around the assignment operator.
        key.leaf_decor_mut().set_suffix(" ");
        match item {
            Item::Value(value) => value.decor_mut().set_prefix(" "),
            Item::Table(table) => format_table(table),
            Item::ArrayOfTables(tables) => {
                for table in tables.iter_mut() {
                    format_table(table);
                }
            }
            Item::None => {}
        }
    }
}

#[salsa::tracked(returns(clone))]
fn manifest_index(
    db: &dyn base_db::source_db::SourceDb,
    key: SourceFileQueryKey,
) -> Arc<ManifestIndex> {
    let file_id = key.file_id(db);
    let text = db.file_text(file_id);
    let (entries, error, formatted_text) = match parse_document(&text) {
        Ok((entries, mut document)) => {
            format_document(&mut document);
            (entries, None, Some(document.to_string()))
        }
        Err(error) => {
            tracing::debug!(?file_id, error = %error, "vide.toml parsed with errors");
            (
                Vec::new(),
                Some(ManifestParseError {
                    range: error.span().and_then(text_range),
                    message: error.to_string(),
                }),
                None,
            )
        }
    };
    Arc::new(ManifestIndex { entries, error, formatted_text })
}

fn index_for(db: &dyn base_db::source_db::SourceDb, file_id: FileId) -> Option<Arc<ManifestIndex>> {
    is_manifest(db, file_id).then(|| manifest_index(db, SourceFileQueryKey::new(db, file_id)))
}

fn entries_for(db: &dyn SourceDb, file_id: FileId) -> Option<Vec<ManifestEntry>> {
    Some(index_for(db, file_id)?.entries.clone())
}

fn key_range(text: &str, value_start: TextSize) -> Option<TextRange> {
    let value_start: usize = value_start.into();
    let line_start = text[..value_start].rfind('\n').map_or(0, |idx| idx + 1);
    let line = &text[line_start..value_start];
    let equals = line.rfind('=')?;
    let key = line[..equals].trim();
    if key.is_empty() {
        return None;
    }
    let key_start = line_start + line[..equals].find(key)?;
    text_range(key_start..key_start + key.len())
}

fn item_values(text: &str, item: &Item) -> Vec<ManifestValue> {
    if let Some(array) = item.as_array() {
        return array.iter().filter_map(|value| manifest_value_value(text, value)).collect();
    }

    manifest_value(text, item).into_iter().collect()
}

fn manifest_value(text: &str, item: &Item) -> Option<ManifestValue> {
    let value = item.as_value()?;
    manifest_value_value(text, value)
}

fn manifest_value_value(text: &str, value: &Value) -> Option<ManifestValue> {
    let range = value.span().and_then(text_range)?;
    let kind = if value.as_str().is_some() {
        ManifestValueKind::String
    } else if value.as_integer().is_some() {
        ManifestValueKind::Integer
    } else if value.as_bool().is_some() {
        ManifestValueKind::Boolean
    } else {
        ManifestValueKind::Other
    };
    let raw = text.get(usize::from(range.start())..usize::from(range.end()))?;
    let text_value = value.as_str().map(ToOwned::to_owned).unwrap_or_else(|| raw.trim().to_owned());
    let content_range = string_content_range(raw, range.start()).unwrap_or(range);
    Some(ManifestValue { text: text_value, range, content_range, kind })
}

fn string_content_range(raw: &str, base: TextSize) -> Option<TextRange> {
    let leading = raw.len() - raw.trim_start().len();
    let raw = raw.trim();
    let quote = *raw.as_bytes().first()?;
    if !matches!(quote, b'"' | b'\'') || raw.len() < 2 {
        return None;
    }
    let end = raw.len() - 1;
    text_range(usize::from(base) + leading + 1..usize::from(base) + leading + end)
}

fn entry_at_index(
    entries: &[ManifestEntry],
    offset: TextSize,
) -> Option<(usize, &ManifestEntry, Option<usize>, Option<&ManifestValue>)> {
    entries.iter().enumerate().find_map(|(entry_index, entry)| {
        if entry.key_range.contains(offset) || entry.value_range.contains(offset) {
            let value_index = entry.values.iter().position(|value| value.range.contains(offset));
            let value = value_index.and_then(|index| entry.values.get(index));
            return Some((entry_index, entry, value_index, value));
        }
        None
    })
}

fn entry_at(
    entries: &[ManifestEntry],
    offset: TextSize,
) -> Option<(&ManifestEntry, Option<&ManifestValue>)> {
    let (_, entry, _, value) = entry_at_index(entries, offset)?;
    Some((entry, value))
}

pub(crate) fn target_at(
    db: &dyn SourceDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<ManifestTarget> {
    let index = index_for(db, file_id)?;
    let (entry_index, _, value_index, _) = entry_at_index(&index.entries, offset)?;
    Some(ManifestTarget { file_id, entry_index, value_index })
}

pub(crate) fn target_info(db: &dyn SourceDb, target: ManifestTarget) -> Option<ManifestTargetInfo> {
    let index = index_for(db, target.file_id)?;
    let entry = index.entries.get(target.entry_index)?;
    Some(ManifestTargetInfo {
        file_id: target.file_id,
        key: entry.key.clone(),
        key_range: entry.key_range,
        selected_value: target.value_index.and_then(|index| entry.values.get(index)).cloned(),
        values: entry.values.clone(),
    })
}

fn manifest_path(db: &RootDb, file_id: FileId) -> Option<utils::paths::AbsPathBuf> {
    db.file_path(file_id)
}

fn target_for_path(db: &RootDb, manifest_file_id: FileId, path: &str) -> Option<FileId> {
    if path.contains('*') || path.contains('?') || path.contains('[') {
        return None;
    }
    let root = manifest_path(db, manifest_file_id)?.parent()?.to_owned();
    let path = root.absolutize(utils::paths::Utf8Path::new(path));
    let path = if std::fs::metadata(path.as_path()).is_ok_and(|metadata| metadata.is_dir()) {
        path.join("vide.toml")
    } else {
        path
    };

    db.files().iter().copied().find(|file_id| {
        db.file_path(*file_id).is_some_and(|candidate| candidate.as_path() == path.as_path())
    })
}

fn module_targets(db: &RootDb, name: &str) -> Vec<NavTarget> {
    let mut targets = db
        .files()
        .iter()
        .copied()
        .flat_map(|file_id| db.file_workspace_symbols(file_id).iter().cloned().collect::<Vec<_>>())
        .filter(|symbol| symbol.kind == SymbolKind::Module && symbol.name == name)
        .map(|symbol| NavTarget {
            file_id: symbol.file_id,
            full_range: symbol.full_range,
            focus_range: Some(symbol.focus_range),
            name: Some(symbol.name.into()),
            kind: Some(symbol.kind),
            container_name: symbol.container_name.map(Into::into),
            description: None,
        })
        .collect::<Vec<_>>();
    targets.sort_by_key(|target| (target.file_id.index(), target.focus_or_full_range().start()));
    targets.dedup();
    targets
}

pub(crate) fn definition_target(
    db: &RootDb,
    target: ManifestTarget,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let info = target_info(db, target)?;
    let selected_values = info.selected_value.iter().collect::<Vec<_>>();
    let values = if selected_values.is_empty() {
        info.values.iter().collect::<Vec<_>>()
    } else {
        selected_values
    };
    let targets = match info.key.as_str() {
        "top_modules" => values.iter().flat_map(|value| module_targets(db, &value.text)).collect(),
        "defines" => values
            .iter()
            .map(|value| NavTarget {
                file_id: info.file_id,
                full_range: value.range,
                focus_range: Some(value.content_range),
                name: Some(value.text.clone().into()),
                kind: None,
                container_name: Some(info.key.clone().into()),
                description: Some("manifest macro definition".to_owned()),
            })
            .collect(),
        "libraries" | "include_dirs" | "sources" => values
            .iter()
            .filter_map(|value| {
                target_for_path(db, info.file_id, &value.text).map(|target_file_id| NavTarget {
                    file_id: target_file_id,
                    full_range: TextRange::empty(TextSize::default()),
                    focus_range: Some(TextRange::empty(TextSize::default())),
                    name: Some(value.text.clone().into()),
                    kind: None,
                    container_name: None,
                    description: db.file_path(target_file_id).map(|path| path.to_string()),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    (!targets.is_empty()).then(|| {
        RangeInfo::new(
            info.selected_value.as_ref().map_or(info.key_range, |value| value.range),
            targets,
        )
    })
}

pub(crate) fn hover_target(db: &RootDb, target: ManifestTarget) -> Option<RangeInfo<Markup>> {
    let info = target_info(db, target)?;
    let range = info.selected_value.as_ref().map_or(info.key_range, |value| value.range);
    let description = MANIFEST_KEYS
        .iter()
        .find(|(key, _)| *key == info.key)
        .map_or("Unknown vide.toml key.", |(_, description)| *description);

    let mut text = format!("**{}**\n\n{}", info.key, description);
    if let Some(value) = info.selected_value {
        if matches!(info.key.as_str(), "top_modules" | "defines") {
            if info.key == "defines" {
                text.push_str(&format!("\n\nMacro definition: `{}`", value.text));
            } else {
                text.push_str(&format!("\n\nSystemVerilog module: `{}`", value.text));
            }
        } else if matches!(info.key.as_str(), "libraries" | "include_dirs" | "sources") {
            if let Some(path) = manifest_path(db, info.file_id).and_then(|path| {
                path.parent()
                    .map(|parent| parent.absolutize(utils::paths::Utf8Path::new(&value.text)))
            }) {
                text.push_str(&format!("\n\nResolved path: `{path}`"));
            }
        }
    }
    Some(RangeInfo::new(range, Markup::from(text)))
}

pub(crate) fn document_symbols(db: &dyn SourceDb, file_id: FileId) -> Vec<DocumentSymbol> {
    let Some(entries) = entries_for(db, file_id) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .map(|entry| DocumentSymbol {
            name: entry.key,
            focus_range: entry.key_range,
            full_range: entry.full_range,
            kind: SymbolKind::Config,
            detail: None,
            container_name: None,
            children: Vec::new(),
        })
        .collect()
}

pub(crate) fn workspace_symbols(
    db: &dyn SourceDb,
    file_ids: &[FileId],
    query: &str,
) -> Vec<crate::workspace_symbols::WorkspaceSymbol> {
    let query = query.to_lowercase();
    file_ids
        .iter()
        .copied()
        .filter(|file_id| is_manifest(db, *file_id))
        .flat_map(|file_id| {
            let query = query.clone();
            document_symbols(db, file_id).into_iter().filter_map(move |symbol| {
                (query.is_empty() || symbol.name.to_lowercase().contains(&query)).then(|| {
                    crate::workspace_symbols::WorkspaceSymbol {
                        file_id,
                        name: symbol.name,
                        focus_range: symbol.focus_range,
                        full_range: symbol.full_range,
                        kind: symbol.kind,
                        container_name: symbol.container_name,
                    }
                })
            })
        })
        .collect()
}

pub(crate) fn highlights_target(
    db: &RootDb,
    target: ManifestTarget,
) -> Option<Vec<DocumentHighlight>> {
    let info = target_info(db, target)?;
    let selected = info.selected_value?.text;
    let ranges = info
        .values
        .iter()
        .filter(|candidate| candidate.text == selected)
        .map(|candidate| DocumentHighlight {
            range: candidate.range,
            category: ReferenceCategory::READ,
        })
        .collect::<Vec<_>>();
    (!ranges.is_empty()).then_some(ranges)
}

pub(crate) fn references_target(
    db: &RootDb,
    target: ManifestTarget,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    let info = target_info(db, target)?;
    let selected_values = info.selected_value.iter().collect::<Vec<_>>();
    let values = if selected_values.is_empty() {
        info.values.iter().collect::<Vec<_>>()
    } else {
        selected_values
    };

    if info.key == "top_modules" {
        let mut module_references = Vec::new();
        for value in &values {
            for module in module_targets(db, &value.text) {
                let Some(mut references) = crate::references::references(
                    db,
                    FilePosition {
                        file_id: module.file_id,
                        offset: module.focus_or_full_range().start(),
                    },
                    config.clone(),
                ) else {
                    continue;
                };
                for references in &mut references {
                    references
                        .refs
                        .entry(info.file_id)
                        .or_default()
                        .push((value.content_range, ReferenceCategory::READ));
                }
                module_references.extend(references);
            }
        }
        tracing::debug!(
            ?info.file_id,
            value_count = values.len(),
            result_count = module_references.len(),
            "vide.toml module references"
        );
        if !module_references.is_empty() {
            return Some(module_references);
        }
    }

    let selected = info.selected_value?;
    let refs = info
        .values
        .iter()
        .filter(|candidate| candidate.text == selected.text)
        .map(|candidate| (candidate.range, ReferenceCategory::READ))
        .collect::<Vec<_>>();
    let mut refs_by_file = nohash_hasher::IntMap::default();
    refs_by_file.insert(info.file_id, refs);
    Some(vec![References {
        def: Some(vec![NavTarget {
            file_id: info.file_id,
            full_range: selected.range,
            focus_range: Some(selected.content_range),
            name: Some(selected.text.clone().into()),
            kind: None,
            container_name: Some(info.key.into()),
            description: Some("vide.toml value".to_owned()),
        }]),
        refs: refs_by_file,
        status: ReferencesStatus::Complete,
    }])
}

pub(crate) fn target_range(db: &RootDb, target: ManifestTarget) -> TextRange {
    target_info(db, target)
        .and_then(|info| {
            info.selected_value.map(|value| value.content_range).or(Some(info.key_range))
        })
        .unwrap_or_default()
}

pub(crate) fn rename_target(
    db: &RootDb,
    target: ManifestTarget,
    config: &crate::rename::RenameConfig,
    new_name: &str,
) -> Result<SourceChange, crate::rename::RenameError> {
    let info = target_info(db, target).ok_or(crate::rename::RenameError::NoRefFound)?;
    let value = info.selected_value.ok_or(crate::rename::RenameError::NoRefFound)?;
    if info.key != "top_modules" {
        let mut builder = SourceChangeBuilder::new(info.file_id);
        builder.replace(value.content_range, new_name);
        return Ok(builder.finish());
    }

    let modules = module_targets(db, &value.text);
    let [module] = modules.as_slice() else {
        tracing::debug!(
            ?info.file_id,
            module = %value.text,
            candidate_count = modules.len(),
            "cannot rename an ambiguous manifest top module"
        );
        return Err(crate::rename::RenameError::NoDefFound);
    };

    let mut source_change = crate::rename::rename(
        db,
        FilePosition { file_id: module.file_id, offset: module.focus_or_full_range().start() },
        config.clone(),
        new_name,
    )?;
    source_change
        .insert_text_edit(info.file_id, TextEdit::replace(value.content_range, new_name.to_owned()))
        .map_err(|_| crate::rename::RenameError::OverlappingEdits)?;
    Ok(source_change)
}

pub(crate) fn rename_module_references(
    db: &RootDb,
    request_file_id: FileId,
    def: &hir_def::def_id::DefId,
    config: &crate::rename::RenameConfig,
    new_name: &str,
    source_change: &mut SourceChange,
) -> Result<(), crate::rename::RenameError> {
    let Some(module_origin) =
        def.origins(db).into_iter().find(|origin| origin.as_module(db).is_some())
    else {
        return Ok(());
    };
    let old_name = module_origin.name(db).ok_or(crate::rename::RenameError::NoRefFound)?;

    for manifest_file_id in db.files() {
        if !is_manifest(db, manifest_file_id) {
            continue;
        }
        let Some(entries) = entries_for(db, manifest_file_id) else {
            continue;
        };
        for entry in entries {
            if entry.key != "top_modules" {
                continue;
            }
            for value in entry.values {
                if value.text != old_name.as_str() {
                    continue;
                }
                if matches!(config.edit_scope(), crate::rename::RenameEditScope::SingleFile)
                    && manifest_file_id != request_file_id
                {
                    return Err(crate::rename::RenameError::ProjectScopeRequired);
                }
                source_change
                    .insert_text_edit(
                        manifest_file_id,
                        TextEdit::replace(value.content_range, new_name.to_owned()),
                    )
                    .map_err(|_| crate::rename::RenameError::OverlappingEdits)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn semantic_tokens(
    db: &dyn SourceDb,
    file_id: FileId,
    range: Option<TextRange>,
) -> Vec<SemaToken> {
    let Some(entries) = entries_for(db, file_id) else {
        return Vec::new();
    };
    let in_range =
        |candidate: TextRange| range.is_none_or(|range| candidate.intersect(range).is_some());
    let mut tokens = Vec::new();
    for entry in entries {
        if in_range(entry.key_range) {
            tokens.push(SemaToken {
                range: entry.key_range,
                tag: SemaTokenTag::TomlKey,
                mods: SemaTokenModifier::DECL,
            });
        }
        for value in entry.values {
            if !in_range(value.range) {
                continue;
            }
            let tag = match value.kind {
                ManifestValueKind::String => SemaTokenTag::TomlString,
                ManifestValueKind::Integer => SemaTokenTag::TomlNumber,
                ManifestValueKind::Boolean => SemaTokenTag::TomlBoolean,
                ManifestValueKind::Other => SemaTokenTag::TomlValue,
            };
            tokens.push(SemaToken { range: value.range, tag, mods: SemaTokenModifier::empty() });
        }
    }
    tokens.extend(comment_tokens(&db.file_text(file_id), range));
    tokens.sort_by_key(|token| token.range.start());
    tokens
}

fn comment_tokens(text: &str, range: Option<TextRange>) -> Vec<SemaToken> {
    let mut comments = Vec::new();
    let mut in_basic = false;
    let mut in_literal = false;
    let mut comment_start = None;
    for (offset, byte) in text.bytes().enumerate() {
        match byte {
            b'"' if !in_literal => in_basic = !in_basic,
            b'\'' if !in_basic => in_literal = !in_literal,
            b'#' if !in_basic && !in_literal => comment_start = Some(offset),
            b'\n' => {
                if let Some(start) = comment_start.take() {
                    push_comment(&mut comments, start, offset, range);
                }
            }
            _ => {}
        }
    }
    if let Some(start) = comment_start {
        push_comment(&mut comments, start, text.len(), range);
    }
    comments
}

fn push_comment(tokens: &mut Vec<SemaToken>, start: usize, end: usize, range: Option<TextRange>) {
    let Some(comment_range) = text_range(start..end) else {
        return;
    };
    if comment_range.is_empty()
        || range.is_some_and(|requested| comment_range.intersect(requested).is_none())
    {
        return;
    }
    tokens.push(SemaToken {
        range: comment_range,
        tag: SemaTokenTag::TomlComment,
        mods: SemaTokenModifier::empty(),
    });
}

pub(crate) fn completions(
    db: &dyn WorkspaceSymbolIndexDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Vec<CompletionItem> {
    let text = db.file_text(file_id);
    let offset_usize: usize = offset.into();
    let word = word_at_offset(&text, offset_usize);
    let entries = match parse(&text) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::debug!(?file_id, error = %error, "using schema completion recovery for invalid vide.toml");
            Vec::new()
        }
    };
    let line_start = text[..offset_usize.min(text.len())].rfind('\n').map_or(0, |idx| idx + 1);
    let before = &text[line_start..offset_usize.min(text.len())];
    if !before.contains('=') {
        let known = entries.iter().map(|entry| entry.key.as_str()).collect::<BTreeSet<_>>();
        return MANIFEST_KEYS
            .iter()
            .filter(|(key, _)| !known.contains(key) && key.starts_with(&word.1))
            .map(|(key, _)| {
                CompletionItem::new(
                    (*key).to_owned(),
                    CompletionItemKind::Keyword,
                    Some(TextEditItem::replace(word.0, (*key).to_owned())),
                    None,
                    format!("0-{key}"),
                )
            })
            .collect();
    }

    let field = entries
        .iter()
        .find(|entry| entry.value_range.contains(offset) || before.contains(entry.key.as_str()))
        .map(|entry| entry.key.as_str());
    if field != Some("top_modules") {
        return Vec::new();
    }

    let mut names = BTreeSet::new();
    for candidate in db.files().iter().copied() {
        for symbol in db.file_workspace_symbols(candidate).iter() {
            if symbol.kind == SymbolKind::Module {
                names.insert(symbol.name.clone());
            }
        }
    }
    names
        .into_iter()
        .filter(|name| name.starts_with(&word.1))
        .map(|name| {
            CompletionItem::new(
                name.clone(),
                CompletionItemKind::Text,
                Some(TextEditItem::replace(word.0, name.clone())),
                None,
                format!("1-{name}"),
            )
        })
        .collect()
}

fn word_at_offset(text: &str, offset: usize) -> (TextRange, String) {
    let offset = offset.min(text.len());
    let is_word = |byte: u8| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'/' | b'-' | b'*' | b'?')
    };
    let mut start = offset;
    while start > 0 && is_word(text.as_bytes()[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < text.len() && is_word(text.as_bytes()[end]) {
        end += 1;
    }
    let range =
        text_range(start..end).unwrap_or_else(|| TextRange::empty(TextSize::from(offset as u32)));
    (range, text[start..offset].to_owned())
}

pub(crate) fn selection_ranges(
    db: &dyn SourceDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Vec<TextRange> {
    let Some(entries) = entries_for(db, file_id) else {
        return vec![TextRange::empty(offset)];
    };
    let Some((entry, value)) = entry_at(&entries, offset) else {
        return vec![TextRange::empty(offset)];
    };
    let mut ranges = vec![TextRange::empty(offset)];
    if let Some(value) = value {
        ranges.push(value.range);
        ranges.push(entry.value_range);
    } else {
        ranges.push(entry.key_range);
    }
    ranges.push(entry.full_range);
    ranges.dedup();
    ranges
}

pub(crate) fn folding_ranges(db: &dyn SourceDb, file_id: FileId) -> Vec<Fold> {
    let Some(entries) = entries_for(db, file_id) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter(|entry| entry.value_range.start() < entry.value_range.end())
        .filter(|entry| {
            let text = db.file_text(file_id);
            line_of_offset(&text, entry.value_range.start())
                < line_of_offset(&text, entry.value_range.end())
        })
        .map(|entry| Fold::new(entry.value_range, FoldKind::Decl))
        .collect()
}

fn line_of_offset(text: &str, offset: TextSize) -> usize {
    text[..usize::from(offset).min(text.len())].bytes().filter(|byte| *byte == b'\n').count()
}

pub(crate) fn format(
    db: &RootDb,
    file_id: FileId,
    cancellation: &utils::cancellation::CancellationToken,
) -> anyhow::Result<Option<utils::text_edit::TextEdit>> {
    cancellation.check()?;
    let index = index_for(db, file_id).ok_or_else(|| anyhow::anyhow!("not a vide.toml file"))?;
    if let Some(error) = &index.error {
        anyhow::bail!(error.message.clone());
    }
    let text = db.file_text(file_id);
    let Some(formatted_text) = &index.formatted_text else {
        anyhow::bail!("vide.toml formatter did not produce a document");
    };
    if formatted_text == text.as_ref() {
        return Ok(None);
    }
    Ok(Some(utils::text_edit::TextEdit::replace(
        TextRange::up_to(TextSize::of(text.as_ref())),
        formatted_text.clone(),
    )))
}

pub(crate) fn diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let Some(index) = index_for(db, file_id) else {
        return Vec::new();
    };
    let Some(error) = &index.error else { return Vec::new() };
    let text = db.file_text(file_id);
    let range = error.range.unwrap_or_else(|| TextRange::up_to(TextSize::of(text.as_ref())));
    vec![Diagnostic {
        file_id,
        code: 100,
        subsystem: 0,
        name: "InvalidToml".to_owned(),
        option_name: None,
        groups: Vec::new(),
        source: DiagnosticSource::Vide,
        range,
        severity: DiagnosticSeverity::Error,
        message: error.message.clone(),
        args: Vec::new(),
        message_key: None,
        message_args: Vec::new(),
        tags: Vec::new(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keeps_key_and_array_item_ranges() {
        let text = "top_modules = [\"top\", \"child\"]\n";
        let entries = parse(text).unwrap();
        assert_eq!(entries[0].key, "top_modules");
        assert_eq!(
            &text[usize::from(entries[0].values[0].range.start())
                ..usize::from(entries[0].values[0].range.end())],
            "\"top\""
        );
        assert_eq!(entries[0].values[0].text, "top");
        assert_eq!(entries[0].values[0].content_range.len(), TextSize::from(3));
    }

    #[test]
    fn key_completion_range_is_the_current_word() {
        let (range, prefix) = word_at_offset("sou", 3);
        assert_eq!(prefix, "sou");
        assert_eq!(range, TextRange::new(TextSize::from(0), TextSize::from(3)));
    }

    #[test]
    fn formatting_normalizes_assignment_spacing_without_dropping_comments() {
        let text = "# project\ntop_modules=[\"top\"] # selected top\n";
        let (_, mut document) = parse_document(text).unwrap();
        format_document(&mut document);
        assert_eq!(document.to_string(), "# project\ntop_modules = [\"top\"] # selected top\n");
    }
}
