use ::preproc::source::{
    PreprocSourceId, SourceEmittedTokenId, SourceEmittedTokenRange, SourceMacroCallId,
    SourceMacroExpansionId, SourceMacroReferenceId, SourcePosition, SourcePreprocError,
    SourcePreprocModel, SourcePreprocUnavailable, SourceRange, SourceTokenProvenance,
};
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use syntax::{PreprocessorTrace, SourceBufferOrigin, SyntaxTreeOptions};
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    path_identity::PathIdentityIndex,
    uniq_vec::UniqVec,
};
use vfs::{FileId, VfsPath};

use super::{SourceFileKind, SourceRootDb, path_file_ids, syntax_tree_options_for_file};
use crate::base_db::project::CompilationProfileId;

mod source_mapping;

#[cfg(not(test))]
use self::source_mapping::source_preproc_file_ids;
use self::source_mapping::{
    display_only_expansion_source_buffer_error, emitted_range_from_token_ranges,
    record_expansion_display_texts, shift_text_range, unshift_text_size,
};
#[cfg(test)]
pub(super) use self::source_mapping::{materialized_predefine_text, source_preproc_file_ids};
pub use self::source_mapping::{
    preproc_virtual_builtin_path, preproc_virtual_expansion_path, preproc_virtual_predefines_path,
    preproc_virtual_speculative_path,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedSourcePreprocModel {
    pub model: SourcePreprocModel,
    pub source_map: PreprocSourceMap,
    range_index: PreprocRangeIndex,
}

impl MappedSourcePreprocModel {
    pub(crate) fn macro_reference_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<SourceMacroReferenceId> {
        self.range_index.reference_ids_at(file_id, offset)
    }

    pub(crate) fn macro_reference_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroReferenceId> {
        self.range_index.reference_ids_intersecting_range(file_id, range)
    }

    pub(crate) fn macro_call_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<SourceMacroCallId> {
        self.range_index.call_ids_at(file_id, offset)
    }

    pub(crate) fn macro_call_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroCallId> {
        self.range_index.call_ids_intersecting_range(file_id, range)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PreprocRangeIndex {
    references_by_file: FxHashMap<FileId, Vec<IndexedRange<SourceMacroReferenceId>>>,
    calls_by_file: FxHashMap<FileId, Vec<IndexedRange<SourceMacroCallId>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexedRange<T> {
    range: TextRange,
    id: T,
}

impl PreprocRangeIndex {
    fn from_model(model: &SourcePreprocModel, source_map: &PreprocSourceMap) -> Self {
        let mut index = Self::default();
        for reference in model.macro_references().iter() {
            if let Some((file_id, range)) = mapped_file_range(source_map, reference.name_range) {
                index
                    .references_by_file
                    .entry(file_id)
                    .or_default()
                    .push(IndexedRange { range, id: reference.id });
            }
        }
        for call in model.macro_calls().iter() {
            if let Some((file_id, range)) = mapped_file_range(source_map, call.call_range) {
                index
                    .calls_by_file
                    .entry(file_id)
                    .or_default()
                    .push(IndexedRange { range, id: call.id });
            }
        }
        for references in index.references_by_file.values_mut() {
            sort_indexed_ranges(references);
        }
        for calls in index.calls_by_file.values_mut() {
            sort_indexed_ranges(calls);
        }
        index
    }

    fn reference_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroReferenceId> {
        ids_at(self.references_by_file.get(&file_id), offset)
    }

    fn reference_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroReferenceId> {
        ids_intersecting_range(self.references_by_file.get(&file_id), range)
    }

    fn call_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroCallId> {
        ids_at(self.calls_by_file.get(&file_id), offset)
    }

    fn call_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroCallId> {
        ids_intersecting_range(self.calls_by_file.get(&file_id), range)
    }
}

fn mapped_file_range(
    source_map: &PreprocSourceMap,
    source_range: SourceRange,
) -> Option<(FileId, TextRange)> {
    let range = source_map.map_range(source_range).ok()?;
    let file_id = source_map.file_id(source_range.source).ok()?;
    Some((file_id, range))
}

fn sort_indexed_ranges<T: Copy>(ranges: &mut [IndexedRange<T>]) {
    ranges.sort_by_key(|entry| (entry.range.start(), entry.range.end()));
}

fn ids_at<T: Copy>(ranges: Option<&Vec<IndexedRange<T>>>, offset: TextSize) -> Vec<T> {
    let Some(ranges) = ranges else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for entry in ranges {
        if entry.range.start() > offset {
            break;
        }
        if entry.range.contains(offset) {
            ids.push(entry.id);
        }
    }
    ids
}

fn ids_intersecting_range<T: Copy>(
    ranges: Option<&Vec<IndexedRange<T>>>,
    range: TextRange,
) -> Vec<T> {
    let Some(ranges) = ranges else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for entry in ranges {
        if entry.range.start() >= range.end() {
            break;
        }
        if entry.range.intersect(range).is_some_and(|intersection| !intersection.is_empty()) {
            ids.push(entry.id);
        }
    }
    ids
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocSourceMap {
    entries: FxHashMap<PreprocSourceId, PreprocSourceMapping>,
    expansion_entries: FxHashMap<SourceMacroExpansionId, PreprocExpansionMapping>,
    predefine_sources: FxHashMap<PreprocSourceId, PreprocManifestSource>,
    text_lengths: FxHashMap<PreprocSourceId, usize>,
    range_offsets: FxHashMap<PreprocSourceId, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocRelevantContexts {
    pub model_file_ids: Vec<FileId>,
    pub status: SourcePreprocContextStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePreprocContextIndex {
    contexts_by_file: FxHashMap<FileId, Vec<FileId>>,
    status: SourcePreprocContextStatus,
}

impl SourcePreprocContextIndex {
    fn contexts_for_file(&self, file_id: FileId) -> SourcePreprocRelevantContexts {
        SourcePreprocRelevantContexts {
            model_file_ids: self.contexts_by_file.get(&file_id).cloned().unwrap_or_default(),
            status: self.status,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourcePreprocContextStatus {
    #[default]
    Complete,
    Partial {
        skipped_models: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocSourceMapping {
    RealFile(FileId),
    VirtualFile { file_id: FileId, path: VfsPath, origin: PreprocVirtualOrigin },
    VirtualDisplay { path: VfsPath, origin: PreprocVirtualOrigin },
    Unmapped(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocExpansionMapping {
    pub origin: PreprocVirtualOrigin,
    pub emitted_range: SourceEmittedTokenRange,
    pub display: PreprocExpansionDisplay,
    pub source_buffer: PreprocExpansionSourceBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocExpansionDisplay {
    pub path: VfsPath,
    pub text: String,
    token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocExpansionSourceBuffer {
    ParseStable {
        file_id: FileId,
        path: VfsPath,
        text: String,
        token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
    },
    DisplayOnly {
        path: VfsPath,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreprocManifestSource {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocVirtualOrigin {
    Predefines { profile: Option<CompilationProfileId> },
    Builtin { name: SmolStr },
    ExternalIncludeBuffer { source: PreprocSourceId },
    Expansion { expansion: SourceMacroExpansionId },
    Speculative { universe: PreprocSpeculativeUniverseId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreprocSpeculativeUniverseId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocSourceMapError {
    MissingSource {
        source: PreprocSourceId,
    },
    UnmappedSource {
        source: PreprocSourceId,
        reason: SourcePreprocUnavailable,
    },
    RangeOutOfBounds {
        source: PreprocSourceId,
        range: TextRange,
        mapped_range: TextRange,
        text_len: usize,
    },
    MissingExpansionVirtualFile {
        expansion: SourceMacroExpansionId,
    },
    MissingEmittedToken {
        token: SourceEmittedTokenId,
    },
    MissingEmittedTokenRange {
        range: SourceEmittedTokenRange,
    },
    DisplayOnlyVirtualSource {
        path: VfsPath,
        origin: PreprocVirtualOrigin,
    },
}

impl PreprocSourceMap {
    pub fn insert_real_file(&mut self, source: PreprocSourceId, file_id: FileId, text_len: usize) {
        self.entries.insert(source, PreprocSourceMapping::RealFile(file_id));
        self.predefine_sources.remove(&source);
        self.text_lengths.insert(source, text_len);
        self.range_offsets.insert(source, 0);
    }

    pub fn insert_virtual_file(
        &mut self,
        source: PreprocSourceId,
        file_id: Option<FileId>,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
    ) {
        self.insert_virtual_file_with_offset(source, file_id, path, origin, text_len, 0);
    }

    fn insert_virtual_file_with_offset(
        &mut self,
        source: PreprocSourceId,
        file_id: Option<FileId>,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
        range_offset: usize,
    ) {
        let mapping = match file_id {
            Some(file_id) => PreprocSourceMapping::VirtualFile { file_id, path, origin },
            None => PreprocSourceMapping::VirtualDisplay { path, origin },
        };
        self.entries.insert(source, mapping);
        self.predefine_sources.remove(&source);
        self.text_lengths.insert(source, text_len);
        self.range_offsets.insert(source, range_offset);
    }

    pub fn insert_unmapped(&mut self, source: PreprocSourceId, reason: SourcePreprocUnavailable) {
        self.entries.insert(source, PreprocSourceMapping::Unmapped(reason));
        self.predefine_sources.remove(&source);
        self.text_lengths.remove(&source);
        self.range_offsets.remove(&source);
    }

    fn insert_predefine_manifest_source(
        &mut self,
        source: PreprocSourceId,
        manifest_source: PreprocManifestSource,
    ) {
        self.predefine_sources.insert(source, manifest_source);
    }

    pub fn get(&self, source: PreprocSourceId) -> Option<&PreprocSourceMapping> {
        self.entries.get(&source)
    }

    pub fn predefine_manifest_source(
        &self,
        source: PreprocSourceId,
    ) -> Option<PreprocManifestSource> {
        self.predefine_sources.get(&source).copied()
    }

    pub fn insert_expansion_display_only(
        &mut self,
        expansion: SourceMacroExpansionId,
        path: VfsPath,
        display_text: String,
        emitted_range: SourceEmittedTokenRange,
        display_token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
    ) {
        self.expansion_entries.insert(
            expansion,
            PreprocExpansionMapping {
                origin: PreprocVirtualOrigin::Expansion { expansion },
                emitted_range,
                display: PreprocExpansionDisplay {
                    path: path.clone(),
                    text: display_text,
                    token_ranges: display_token_ranges,
                },
                source_buffer: PreprocExpansionSourceBuffer::DisplayOnly { path },
            },
        );
    }

    pub fn expansion(&self, expansion: SourceMacroExpansionId) -> Option<&PreprocExpansionMapping> {
        self.expansion_entries.get(&expansion)
    }

    pub fn expansion_display_source(
        &self,
        expansion: SourceMacroExpansionId,
    ) -> Result<PreprocSourceMapping, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        Ok(PreprocSourceMapping::VirtualDisplay {
            path: entry.display.path.clone(),
            origin: entry.origin.clone(),
        })
    }

    pub fn expansion_source_buffer(
        &self,
        expansion: SourceMacroExpansionId,
    ) -> Result<PreprocSourceMapping, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        Ok(match &entry.source_buffer {
            PreprocExpansionSourceBuffer::ParseStable { file_id, path, .. } => {
                PreprocSourceMapping::VirtualFile {
                    file_id: *file_id,
                    path: path.clone(),
                    origin: entry.origin.clone(),
                }
            }
            PreprocExpansionSourceBuffer::DisplayOnly { path } => {
                PreprocSourceMapping::VirtualDisplay {
                    path: path.clone(),
                    origin: entry.origin.clone(),
                }
            }
        })
    }

    pub fn emitted_display_range(
        &self,
        expansion: SourceMacroExpansionId,
        emitted_range: SourceEmittedTokenRange,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        emitted_range_from_token_ranges(&entry.display.token_ranges, emitted_range)
            .ok_or(PreprocSourceMapError::MissingEmittedTokenRange { range: emitted_range })
    }

    pub fn emitted_source_buffer_range(
        &self,
        expansion: SourceMacroExpansionId,
        emitted_range: SourceEmittedTokenRange,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        let PreprocExpansionSourceBuffer::ParseStable { token_ranges, .. } = &entry.source_buffer
        else {
            return Err(display_only_expansion_source_buffer_error(entry));
        };
        emitted_range_from_token_ranges(token_ranges, emitted_range)
            .ok_or(PreprocSourceMapError::MissingEmittedTokenRange { range: emitted_range })
    }

    pub fn emitted_token_display_range(
        &self,
        expansion: SourceMacroExpansionId,
        token: SourceEmittedTokenId,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        entry
            .display
            .token_ranges
            .get(&token)
            .copied()
            .ok_or(PreprocSourceMapError::MissingEmittedToken { token })
    }

    pub fn emitted_token_source_buffer_range(
        &self,
        expansion: SourceMacroExpansionId,
        token: SourceEmittedTokenId,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        let PreprocExpansionSourceBuffer::ParseStable { token_ranges, .. } = &entry.source_buffer
        else {
            return Err(display_only_expansion_source_buffer_error(entry));
        };
        token_ranges
            .get(&token)
            .copied()
            .ok_or(PreprocSourceMapError::MissingEmittedToken { token })
    }

    pub fn insert_expansion_parse_stable_source_buffer(
        &mut self,
        expansion: SourceMacroExpansionId,
        file_id: FileId,
        path: VfsPath,
        text: String,
        token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
    ) -> Result<(), PreprocSourceMapError> {
        let entry = self
            .expansion_entries
            .get_mut(&expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        entry.source_buffer =
            PreprocExpansionSourceBuffer::ParseStable { file_id, path, text, token_ranges };
        Ok(())
    }

    pub fn expansion_display_text(&self, expansion: SourceMacroExpansionId) -> Option<&str> {
        self.expansion(expansion).map(|entry| entry.display.text.as_str())
    }

    pub fn file_id(&self, source: PreprocSourceId) -> Result<FileId, PreprocSourceMapError> {
        match self.get(source) {
            Some(PreprocSourceMapping::RealFile(file_id)) => Ok(*file_id),
            Some(PreprocSourceMapping::VirtualFile { file_id, .. }) => Ok(*file_id),
            Some(PreprocSourceMapping::VirtualDisplay { path, origin }) => {
                Err(PreprocSourceMapError::DisplayOnlyVirtualSource {
                    path: path.clone(),
                    origin: origin.clone(),
                })
            }
            Some(PreprocSourceMapping::Unmapped(reason)) => {
                Err(PreprocSourceMapError::UnmappedSource { source, reason: reason.clone() })
            }
            None => Err(PreprocSourceMapError::MissingSource { source }),
        }
    }

    pub fn source_positions_for_file_offset(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<SourcePosition> {
        let mut positions = self
            .entries
            .iter()
            .filter_map(|(source, mapping)| {
                let mapped_file_id = match mapping {
                    PreprocSourceMapping::RealFile(mapped_file_id)
                    | PreprocSourceMapping::VirtualFile { file_id: mapped_file_id, .. } => {
                        *mapped_file_id
                    }
                    PreprocSourceMapping::VirtualDisplay { .. } => return None,
                    PreprocSourceMapping::Unmapped(_) => return None,
                };
                if mapped_file_id != file_id {
                    return None;
                }

                let range_offset = self.range_offsets.get(source).copied().unwrap_or(0);
                let source_offset = unshift_text_size(offset, range_offset)?;
                let text_len = self.text_lengths.get(source).copied()?;
                (usize::from(source_offset) <= text_len)
                    .then_some(SourcePosition { source: *source, offset: source_offset })
            })
            .collect::<Vec<_>>();
        positions.sort_by_key(|position| position.source.raw());
        positions
    }

    pub fn map_range(&self, source_range: SourceRange) -> Result<TextRange, PreprocSourceMapError> {
        match self.get(source_range.source) {
            Some(PreprocSourceMapping::RealFile(_))
            | Some(PreprocSourceMapping::VirtualFile { .. })
            | Some(PreprocSourceMapping::VirtualDisplay { .. }) => {}
            Some(PreprocSourceMapping::Unmapped(reason)) => {
                return Err(PreprocSourceMapError::UnmappedSource {
                    source: source_range.source,
                    reason: reason.clone(),
                });
            }
            None => {
                return Err(PreprocSourceMapError::MissingSource { source: source_range.source });
            }
        }

        let range_offset = self.range_offsets.get(&source_range.source).copied().unwrap_or(0);
        let mapped_range = shift_text_range(source_range.range, range_offset).ok_or(
            PreprocSourceMapError::RangeOutOfBounds {
                source: source_range.source,
                range: source_range.range,
                mapped_range: source_range.range,
                text_len: usize::MAX,
            },
        )?;
        let text_len = self
            .text_lengths
            .get(&source_range.source)
            .copied()
            .ok_or(PreprocSourceMapError::MissingSource { source: source_range.source })?;
        if usize::from(mapped_range.end()) <= text_len {
            return Ok(mapped_range);
        }

        Err(PreprocSourceMapError::RangeOutOfBounds {
            source: source_range.source,
            range: source_range.range,
            mapped_range,
            text_len,
        })
    }
}

fn preproc_context_file_ids(
    mapped: &MappedSourcePreprocModel,
    model_file_id: FileId,
) -> Vec<FileId> {
    let mut file_ids = UniqVec::<FileId, FileId>::default();
    file_ids.push_unique(model_file_id);

    for definition in mapped.model.macro_definitions().iter() {
        collect_context_source_range(mapped, definition.directive_range, &mut file_ids);
        collect_context_source_range(mapped, definition.name_range, &mut file_ids);
        if let Some(params) = &definition.params {
            for param in params {
                if let Some(range) = param.name_range {
                    collect_context_source_range(mapped, range, &mut file_ids);
                }
                if let Some(range) = param.range {
                    collect_context_source_range(mapped, range, &mut file_ids);
                }
                if let Some(default) = &param.default {
                    for token in default {
                        if let Some(range) = token.range {
                            collect_context_source_range(mapped, range, &mut file_ids);
                        }
                    }
                }
            }
        }
        for token in &definition.body_tokens {
            if let Some(range) = token.range {
                collect_context_source_range(mapped, range, &mut file_ids);
            }
        }
    }

    for reference in mapped.model.macro_references().iter() {
        collect_context_source_range(mapped, reference.directive_range, &mut file_ids);
        collect_context_source_range(mapped, reference.name_range, &mut file_ids);
    }

    for call in mapped.model.macro_calls().iter() {
        collect_context_source_range(mapped, call.call_range, &mut file_ids);
        for argument in &call.arguments {
            if let Some(range) = argument.argument_range {
                collect_context_source_range(mapped, range, &mut file_ids);
            }
            for token in &argument.tokens {
                if let Some(range) = token.range {
                    collect_context_source_range(mapped, range, &mut file_ids);
                }
            }
        }
    }

    for include in mapped.model.include_graph().directives() {
        collect_context_source_range(mapped, include.directive_range, &mut file_ids);
        if let Some(range) = include.target_range {
            collect_context_source_range(mapped, range, &mut file_ids);
        }
        if let Some(source) = include.resolved_source {
            collect_context_source(mapped, source, &mut file_ids);
        }
    }

    for range in mapped.model.inactive_ranges() {
        collect_context_source_range(mapped, *range, &mut file_ids);
    }

    for provenance in mapped.model.token_provenance().iter() {
        match provenance {
            SourceTokenProvenance::Source { token_range }
            | SourceTokenProvenance::MacroBody { body_token_range: token_range, .. } => {
                collect_context_source_range(mapped, *token_range, &mut file_ids);
            }
            SourceTokenProvenance::MacroArgument {
                body_token_range, argument_token_range, ..
            } => {
                collect_context_source_range(mapped, *body_token_range, &mut file_ids);
                collect_context_source_range(mapped, *argument_token_range, &mut file_ids);
            }
            SourceTokenProvenance::TokenPaste { .. }
            | SourceTokenProvenance::Stringification { .. }
            | SourceTokenProvenance::Builtin { .. }
            | SourceTokenProvenance::Unavailable(_) => {}
            SourceTokenProvenance::Predefine { source } => {
                collect_context_source(mapped, *source, &mut file_ids);
            }
        }
    }

    let mut file_ids = file_ids.into_vec();
    file_ids.sort();
    file_ids
}

fn collect_context_source_range(
    mapped: &MappedSourcePreprocModel,
    range: SourceRange,
    file_ids: &mut UniqVec<FileId, FileId>,
) {
    collect_context_source(mapped, range.source, file_ids);
}

fn collect_context_source(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
    file_ids: &mut UniqVec<FileId, FileId>,
) {
    if let Ok(file_id) = mapped.source_map.file_id(source) {
        file_ids.push_unique(file_id);
    }
    if let Some(manifest_source) = mapped.source_map.predefine_manifest_source(source) {
        file_ids.push_unique(manifest_source.file_id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocQueryError {
    UnsupportedFileKind(SourceFileKind),
    TraceUnavailable,
    Model(SourcePreprocError),
    UnmappedSource { buffer_id: u32, path: String },
}

pub(crate) fn workspace_preproc_model_file_ids(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Vec<FileId> {
    let plan = db.compilation_plan_for_profile(profile_id);
    let mut file_ids = FxHashSet::default();

    for root in plan.roots.iter().copied() {
        if matches!(
            db.file_kind(root),
            SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader
        ) {
            file_ids.insert(root);
        }
    }
    file_ids.extend(plan.include_only.iter().copied());

    for source_root_id in &plan.source_roots {
        for candidate in db.source_root(*source_root_id).iter() {
            if db.file_is_project_ignored(candidate) {
                continue;
            }
            if matches!(db.file_kind(candidate), SourceFileKind::IncludeHeader) {
                file_ids.insert(candidate);
            }
        }
    }

    for candidate in db.files().iter().copied() {
        if db.file_is_project_ignored(candidate) {
            continue;
        }
        if !matches!(db.file_kind(candidate), SourceFileKind::IncludeHeader) {
            continue;
        }
        let Some(path) = db.file_path(candidate) else {
            continue;
        };
        if plan.include_dirs.iter().any(|include_dir| path.starts_with(include_dir)) {
            file_ids.insert(candidate);
        }
    }

    let mut file_ids = file_ids.into_iter().collect::<Vec<_>>();
    file_ids.sort();
    file_ids
}

pub(super) fn source_preproc_model(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<Result<MappedSourcePreprocModel, SourcePreprocQueryError>> {
    let file_kind = db.file_kind(file_id);
    if !matches!(file_kind, SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader) {
        return Arc::new(Err(SourcePreprocQueryError::UnsupportedFileKind(file_kind)));
    }

    let profile_id = db.file_compilation_profile(file_id);
    let preprocess = db.file_preprocess_config(file_id);
    let options = syntax_tree_options_for_file(db, file_id);
    let Some(trace) = db.parsed_compilation_unit(file_id).preprocessor_trace.clone() else {
        return Arc::new(Err(SourcePreprocQueryError::TraceUnavailable));
    };

    let mut source_map =
        match source_preproc_file_ids(db, file_id, profile_id, &trace, &options, &preprocess) {
            Ok(source_map) => source_map,
            Err(err) => return Arc::new(Err(err)),
        };
    let model = match SourcePreprocModel::from_trace(trace) {
        Ok(model) => model,
        Err(err) => return Arc::new(Err(SourcePreprocQueryError::Model(err))),
    };
    record_expansion_display_texts(profile_id, &model, &mut source_map);
    let range_index = PreprocRangeIndex::from_model(&model, &source_map);

    Arc::new(Ok(MappedSourcePreprocModel { model, source_map, range_index }))
}

pub(super) fn source_preproc_context_index_for_profile(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<SourcePreprocContextIndex> {
    let plan = db.compilation_plan_for_profile(profile_id);
    let mut contexts_by_file = FxHashMap::<FileId, UniqVec<FileId, FileId>>::default();
    let mut skipped_models = 0usize;

    for model_file_id in plan.roots.iter().copied() {
        if !matches!(
            db.file_kind(model_file_id),
            SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader
        ) {
            continue;
        }
        let mapped = db.source_preproc_model(model_file_id);
        match mapped.as_ref() {
            Ok(mapped) => {
                for file_id in preproc_context_file_ids(mapped, model_file_id) {
                    if file_id == model_file_id {
                        continue;
                    }
                    contexts_by_file.entry(file_id).or_default().push_unique(model_file_id);
                }
            }
            Err(_) => skipped_models += 1,
        }
    }

    let contexts_by_file = contexts_by_file
        .into_iter()
        .map(|(file_id, model_file_ids)| {
            let mut model_file_ids = model_file_ids.into_vec();
            model_file_ids.sort();
            (file_id, model_file_ids)
        })
        .collect();
    let status = if skipped_models == 0 {
        SourcePreprocContextStatus::Complete
    } else {
        SourcePreprocContextStatus::Partial { skipped_models }
    };
    Arc::new(SourcePreprocContextIndex { contexts_by_file, status })
}

pub(super) fn source_preproc_contexts_for_file(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<SourcePreprocRelevantContexts> {
    let profile_id = db.file_compilation_profile(file_id);
    Arc::new(db.source_preproc_context_index_for_profile(profile_id).contexts_for_file(file_id))
}
