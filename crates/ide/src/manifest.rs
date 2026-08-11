//! IDE support for the `vide.toml` project manifest.
//!
//! Project manifests are deliberately not SystemVerilog parse units.  They
//! still need source-aware editor features, though, so this module owns the
//! small TOML model used by those features.  `toml_parser` supplies syntax
//! events and exact source spans; `toml_edit` supplies decoded logical values
//! and exact value spans, including for unsaved editor content.

use std::{collections::BTreeSet, ops::Range};

use base_db::source_db::SourceDb;
use hir_def::container::InFile;
use preproc_expand::source_db::manifest_predefine_name_range_in_text;
use syntax::DiagnosticSeverity;
use toml_edit::{ImDocument, Item, Value};
use toml_parser::{Source, Span, lexer::TokenKind, parser::EventKind};
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    text_edit::{TextEdit, TextEditItem},
};
use vfs::FileId;

use crate::{
    DefKind, FilePosition, RangeInfo,
    completion::{CompletionItem, CompletionItemKind},
    db::{SourceFileQueryKey, root_db::RootDb, workspace_symbol_index_db::WorkspaceSymbolIndexDb},
    diagnostics::{Diagnostic, DiagnosticSource},
    document_highlight::DocumentHighlight,
    document_symbols::DocumentSymbol,
    folding_ranges::{Fold, FoldKind},
    markup::Markup,
    navigation_target::NavTarget,
    references::{ReferenceCategory, References, ReferencesConfig},
    semantic_tokens::{SemaToken, SemaTokenModifier, SemaTokenTag},
    source_change::SourceChange,
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
    edit_range: Option<TextRange>,
    semantic_range: Option<TextRange>,
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
    db.file_kind(file_id).is_project_manifest()
}

fn text_range(range: Range<usize>) -> Option<TextRange> {
    Some(TextRange::new(
        TextSize::from(u32::try_from(range.start).ok()?),
        TextSize::from(u32::try_from(range.end).ok()?),
    ))
}

fn parse_document(text: &str) -> Result<Vec<ManifestEntry>, ManifestParseError> {
    let document = ImDocument::parse(text.to_owned()).map_err(|error| ManifestParseError {
        range: error.span().and_then(text_range),
        message: error.to_string(),
    })?;
    let key_ranges = parser_key_ranges(text)?;
    let mut entries = Vec::new();
    let mut key_ranges = key_ranges.into_iter();

    for (key, item) in document.iter() {
        if item.as_value().is_none() {
            return Err(ManifestParseError {
                range: item.span().and_then(text_range),
                message: format!("nested TOML table `{key}` is not supported in vide.toml"),
            });
        }
        let Some(value_range) = item.span().and_then(text_range) else {
            tracing::error!(key = %key, "toml_edit returned an entry without a source span");
            return Err(ManifestParseError {
                range: None,
                message: format!("TOML entry `{key}` has no source span"),
            });
        };
        let Some(key_range) = key_ranges.next() else {
            return Err(ManifestParseError {
                range: Some(value_range),
                message: format!("TOML parser returned no key span for `{key}`"),
            });
        };
        let full_range = TextRange::new(key_range.start(), value_range.end());
        let values = item_values(text, key, item);
        entries.push(ManifestEntry {
            key: key.to_owned(),
            key_range,
            value_range,
            full_range,
            values,
        });
    }

    if key_ranges.next().is_some() {
        return Err(ManifestParseError {
            range: None,
            message: "TOML parser returned more key spans than toml_edit".to_owned(),
        });
    }
    entries.sort_by_key(|entry| entry.full_range.start());
    Ok(entries)
}

#[derive(Debug, Default)]
struct ManifestParserSyntax {
    key_ranges: Vec<TextRange>,
    incomplete_key_range: Option<TextRange>,
}

fn parser_syntax(text: &str) -> Result<(ManifestParserSyntax, bool), ManifestParseError> {
    let source = Source::new(text);
    let tokens = source.lex().into_vec();
    let mut events = Vec::new();
    let mut errors = Vec::new();
    toml_parser::parser::parse_document(&tokens, &mut events, &mut errors);

    let mut syntax = ManifestParserSyntax::default();
    let mut pending_key: Option<TextRange> = None;
    let mut container_depth = 0usize;
    for event in events {
        match event.kind() {
            EventKind::StdTableOpen
            | EventKind::ArrayTableOpen
            | EventKind::InlineTableOpen
            | EventKind::ArrayOpen => container_depth += 1,
            EventKind::StdTableClose
            | EventKind::ArrayTableClose
            | EventKind::InlineTableClose
            | EventKind::ArrayClose => {
                container_depth = container_depth.checked_sub(1).ok_or(ManifestParseError {
                    range: Some(text_range_from_span(event.span())?),
                    message: "TOML parser emitted an unmatched container close".to_owned(),
                })?
            }
            EventKind::SimpleKey if container_depth == 0 => {
                let span = text_range_from_span(event.span())?;
                pending_key = Some(match pending_key {
                    Some(range) => TextRange::new(range.start(), span.end()),
                    None => span,
                });
            }
            EventKind::KeyValSep if container_depth == 0 => {
                let range = pending_key.take().ok_or(ManifestParseError {
                    range: Some(text_range_from_span(event.span())?),
                    message: "TOML parser emitted a key/value separator without a key".to_owned(),
                })?;
                syntax.key_ranges.push(range);
            }
            EventKind::Newline => syntax.incomplete_key_range = pending_key.take(),
            _ => {}
        }
    }
    syntax.incomplete_key_range = pending_key.or(syntax.incomplete_key_range);
    Ok((syntax, !errors.is_empty()))
}

fn parser_key_ranges(text: &str) -> Result<Vec<TextRange>, ManifestParseError> {
    let (syntax, has_errors) = parser_syntax(text)?;
    if has_errors {
        return Err(ManifestParseError {
            range: None,
            message: "TOML parser rejected the document while producing source spans".to_owned(),
        });
    }
    Ok(syntax.key_ranges)
}

fn text_range_from_span(span: Span) -> Result<TextRange, ManifestParseError> {
    text_range(span.start()..span.end()).ok_or(ManifestParseError {
        range: None,
        message: "TOML parser returned a span outside the supported text range".to_owned(),
    })
}

#[salsa::tracked(returns(clone))]
fn manifest_index(
    db: &dyn base_db::source_db::SourceDb,
    key: SourceFileQueryKey,
) -> Arc<ManifestIndex> {
    let file_id = key.file_id(db);
    let text = db.file_text(file_id);
    let (entries, error) = match parse_document(&text) {
        Ok(entries) => (entries, None),
        Err(error) => {
            tracing::debug!(?file_id, error = %error.message, "vide.toml parsed with errors");
            (Vec::new(), Some(ManifestParseError { range: error.range, message: error.message }))
        }
    };
    Arc::new(ManifestIndex { entries, error })
}

fn index_for(db: &dyn base_db::source_db::SourceDb, file_id: FileId) -> Option<Arc<ManifestIndex>> {
    is_manifest(db, file_id).then(|| manifest_index(db, SourceFileQueryKey::new(db, file_id)))
}

fn entries_for(db: &dyn SourceDb, file_id: FileId) -> Option<Vec<ManifestEntry>> {
    Some(index_for(db, file_id)?.entries.clone())
}

fn item_values(text: &str, key: &str, item: &Item) -> Vec<ManifestValue> {
    if let Some(array) = item.as_array() {
        return array.iter().filter_map(|value| manifest_value_value(text, key, value)).collect();
    }

    manifest_value(text, key, item).into_iter().collect()
}

fn manifest_value(text: &str, key: &str, item: &Item) -> Option<ManifestValue> {
    let value = item.as_value()?;
    manifest_value_value(text, key, value)
}

fn manifest_value_value(text: &str, key: &str, value: &Value) -> Option<ManifestValue> {
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
    let text_value = match value.as_str() {
        Some(value) => value.to_owned(),
        None => raw.to_owned(),
    };
    let edit_range = value.as_str().map(|_| range);
    let semantic_range = if key == "defines" {
        manifest_predefine_name_range_in_text(text, range)
    } else {
        Some(range)
    };
    Some(ManifestValue { text: text_value, range, edit_range, semantic_range, kind })
}

fn entry_at_index(
    entries: &[ManifestEntry],
    offset: TextSize,
) -> Option<(usize, &ManifestEntry, Option<usize>, Option<&ManifestValue>)> {
    entries.iter().enumerate().find_map(|(entry_index, entry)| {
        if entry.key_range.contains(offset) || entry.value_range.contains(offset) {
            let value_index = entry
                .values
                .iter()
                .position(|value| value.semantic_range.is_some_and(|range| range.contains(offset)));
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
    let (entry_index, entry, value_index, _) = entry_at_index(&index.entries, offset)?;
    if value_index.is_none() && entry.value_range.contains(offset) {
        return None;
    }
    Some(ManifestTarget { file_id, entry_index, value_index })
}

pub(crate) fn target_capabilities(
    db: &dyn SourceDb,
    target: ManifestTarget,
) -> crate::semantic_target::TargetCapability {
    let Some(info) = target_info(db, target) else {
        return crate::semantic_target::TargetCapability::empty();
    };
    let mut capabilities = crate::semantic_target::TargetCapability::DESCRIBE;
    let Some(value) = info.selected_value else {
        if info.key == "top_modules" && !info.values.is_empty() {
            capabilities |= crate::semantic_target::TargetCapability::NAVIGATE;
        }
        return capabilities;
    };
    let Some(_) = value.semantic_range else {
        return capabilities;
    };

    match info.key.as_str() {
        "top_modules" => {
            capabilities |= crate::semantic_target::TargetCapability::HIGHLIGHT;
            capabilities |= crate::semantic_target::TargetCapability::NAVIGATE
                | crate::semantic_target::TargetCapability::REFERENCES
                | crate::semantic_target::TargetCapability::RENAME;
        }
        "defines" => {
            capabilities |= crate::semantic_target::TargetCapability::NAVIGATE
                | crate::semantic_target::TargetCapability::HIGHLIGHT;
        }
        "libraries" | "include_dirs" | "sources" => {
            capabilities |= crate::semantic_target::TargetCapability::NAVIGATE;
        }
        _ => {}
    }
    capabilities
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
    let root = manifest_path(db, manifest_file_id)?.parent()?.to_owned();
    let path = root.absolutize(utils::paths::Utf8Path::new(path));

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
        .filter(|symbol| symbol.kind == DefKind::Module && symbol.name == name)
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
    let selected = info.selected_value.as_ref();
    let values = match (info.key.as_str(), selected) {
        ("top_modules", Some(value)) => vec![value],
        ("top_modules", None) => info.values.iter().collect(),
        _ => vec![selected?],
    };
    let targets = match info.key.as_str() {
        "top_modules" => {
            Some(values.iter().flat_map(|value| module_targets(db, &value.text)).collect())
        }
        "defines" => {
            let value = values[0];
            let semantic_range = value.semantic_range?;
            Some(vec![NavTarget {
                file_id: info.file_id,
                full_range: value.range,
                focus_range: Some(semantic_range),
                name: Some(value.text.clone().into()),
                kind: None,
                container_name: Some(info.key.clone().into()),
                description: Some("manifest macro definition".to_owned()),
            }])
        }
        "libraries" | "include_dirs" | "sources" => {
            let value = values[0];
            target_for_path(db, info.file_id, &value.text).map(|target_file_id| {
                vec![NavTarget {
                    file_id: target_file_id,
                    full_range: TextRange::empty(TextSize::default()),
                    focus_range: Some(TextRange::empty(TextSize::default())),
                    name: Some(value.text.clone().into()),
                    kind: None,
                    container_name: None,
                    description: db.file_path(target_file_id).map(|path| path.to_string()),
                }]
            })
        }
        _ => None,
    }?;
    let range = match selected {
        Some(value) => value.semantic_range?,
        None => info.key_range,
    };
    (!targets.is_empty()).then(|| RangeInfo::new(range, targets))
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
        } else if matches!(info.key.as_str(), "libraries" | "include_dirs" | "sources")
            && target_for_path(db, info.file_id, &value.text).is_some()
            && let Some(path) = manifest_path(db, info.file_id).and_then(|path| {
                path.parent()
                    .map(|parent| parent.absolutize(utils::paths::Utf8Path::new(&value.text)))
            })
        {
            text.push_str(&format!("\n\nResolved path: `{path}`"));
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
            kind: DefKind::Config,
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
                (query.is_empty() || symbol.name.to_lowercase().contains(&query)).then_some({
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
    if !matches!(info.key.as_str(), "top_modules" | "defines") {
        return None;
    }
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
    let selected = info.selected_value?;
    if info.key != "top_modules" {
        return None;
    }
    let modules = module_targets(db, &selected.text);
    let [module] = modules.as_slice() else {
        tracing::debug!(
            ?info.file_id,
            module = %selected.text,
            "vide.toml module reference target is unresolved or ambiguous"
        );
        return None;
    };
    let mut references = crate::references::references(
        db,
        FilePosition { file_id: module.file_id, offset: module.focus_or_full_range().start() },
        config,
    )?;
    for references in &mut references {
        references
            .refs
            .entry(info.file_id)
            .or_default()
            .push((selected.range, ReferenceCategory::READ));
    }
    (!references.is_empty()).then_some(references)
}

pub(crate) fn target_range(db: &RootDb, target: ManifestTarget) -> Option<TextRange> {
    target_info(db, target)
        .and_then(|info| info.selected_value.and_then(|value| value.semantic_range))
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
        return Err(crate::rename::RenameError::NoRefFound);
    }
    let edit_range = value.edit_range.ok_or(crate::rename::RenameError::NoRefFound)?;

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
        .insert_text_edit(info.file_id, TextEdit::replace(edit_range, format!("\"{new_name}\"")))
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
    let Some(InFile { file_id: origin_file, value: origin_range }) = module_origin.name_range(db)
    else {
        return Err(crate::rename::RenameError::NoRefFound);
    };
    let Some(origin_file) = origin_file.as_file() else {
        return Err(crate::rename::RenameError::NoRefFound);
    };

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
                let modules = module_targets(db, &value.text);
                let [module] = modules.as_slice() else {
                    tracing::debug!(
                        ?manifest_file_id,
                        module = %value.text,
                        "skipping unresolved or ambiguous manifest module reference"
                    );
                    continue;
                };
                if module.file_id != origin_file || module.focus_or_full_range() != origin_range {
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
                        TextEdit::replace(
                            value.edit_range.ok_or(crate::rename::RenameError::NoRefFound)?,
                            format!("\"{new_name}\""),
                        ),
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
    tokens.extend(parser_comment_tokens(&db.file_text(file_id), range));
    tokens.sort_by_key(|token| token.range.start());
    tokens
}

fn parser_comment_tokens(text: &str, range: Option<TextRange>) -> Vec<SemaToken> {
    Source::new(text)
        .lex()
        .filter_map(|token| {
            if token.kind() != TokenKind::Comment {
                return None;
            }
            let span = token.span();
            let Some(comment_range) = text_range(span.start()..span.end()) else {
                tracing::error!(?span, "TOML parser returned an invalid comment span");
                return None;
            };
            if comment_range.is_empty()
                || range.is_some_and(|requested| comment_range.intersect(requested).is_none())
            {
                return None;
            }
            Some(SemaToken {
                range: comment_range,
                tag: SemaTokenTag::TomlComment,
                mods: SemaTokenModifier::empty(),
            })
        })
        .collect()
}

pub(crate) fn completions(
    db: &dyn WorkspaceSymbolIndexDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Vec<CompletionItem> {
    let text = db.file_text(file_id);
    let entries = match parse_document(&text) {
        Ok(entries) => entries,
        Err(_) => {
            let Some(key_range) = parser_incomplete_key_range(&text, offset) else {
                tracing::debug!(?file_id, "vide.toml completion has no parser key context");
                return Vec::new();
            };
            return MANIFEST_KEYS
                .iter()
                .map(|(key, _)| {
                    CompletionItem::new(
                        (*key).to_owned(),
                        CompletionItemKind::Keyword,
                        Some(TextEditItem::replace(key_range, (*key).to_owned())),
                        None,
                        format!("0-{key}"),
                    )
                })
                .collect();
        }
    };
    let Some((_, entry, _, Some(value))) = entry_at_index(&entries, offset) else {
        return Vec::new();
    };
    if entry.key != "top_modules" {
        return Vec::new();
    }

    let mut names = BTreeSet::new();
    for candidate in db.files().iter().copied() {
        for symbol in db.file_workspace_symbols(candidate).iter() {
            if symbol.kind == DefKind::Module {
                names.insert(symbol.name.clone());
            }
        }
    }
    names
        .into_iter()
        .map(|name| {
            CompletionItem::new(
                name.clone(),
                CompletionItemKind::Text,
                Some(TextEditItem::replace(value.range, format!("\"{name}\""))),
                None,
                format!("1-{name}"),
            )
        })
        .collect()
}

fn parser_incomplete_key_range(text: &str, offset: TextSize) -> Option<TextRange> {
    let (syntax, _) = parser_syntax(text).ok()?;
    let candidate = syntax.incomplete_key_range?;
    candidate.contains(offset).then_some(candidate)
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
    is_range_formatting: bool,
    cancellation: &utils::cancellation::CancellationToken,
) -> anyhow::Result<Option<utils::text_edit::TextEdit>> {
    cancellation.check()?;
    if is_range_formatting {
        return Ok(None);
    }
    let index = index_for(db, file_id).ok_or_else(|| anyhow::anyhow!("not a vide.toml file"))?;
    if let Some(error) = &index.error {
        anyhow::bail!(error.message.clone());
    }
    tracing::debug!(?file_id, "vide.toml formatting is unsupported without a TOML formatter");
    Ok(None)
}

pub(crate) fn diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let Some(index) = index_for(db, file_id) else {
        return Vec::new();
    };
    let Some(error) = &index.error else { return Vec::new() };
    let Some(range) = error.range else {
        tracing::error!(?file_id, message = %error.message, "TOML parser returned an error without a source range");
        return Vec::new();
    };
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
        let entries = parse_document(text).unwrap();
        assert_eq!(entries[0].key, "top_modules");
        assert_eq!(entries[0].key_range, TextRange::new(TextSize::default(), TextSize::from(11)));
        assert_eq!(
            &text[usize::from(entries[0].values[0].range.start())
                ..usize::from(entries[0].values[0].range.end())],
            "\"top\""
        );
        assert_eq!(entries[0].values[0].text, "top");
        assert_eq!(
            entries[0].values[0].edit_range,
            Some(TextRange::new(TextSize::from(15), TextSize::from(20)))
        );
    }

    #[test]
    fn comment_tokens_come_from_toml_lexer() {
        let text = "# real\ntop_modules = [\"# not a comment\"] # trailing\n";
        let comments = parser_comment_tokens(text, None);
        assert_eq!(comments.len(), 2);
        assert_eq!(
            &text[usize::from(comments[0].range.start())..usize::from(comments[0].range.end())],
            "# real"
        );
        assert_eq!(
            &text[usize::from(comments[1].range.start())..usize::from(comments[1].range.end())],
            "# trailing"
        );
    }
}
