use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use preproc::source::{
    PreprocSourceId, SourceEmittedTokenId, SourceEmittedTokenRange, SourceMacroExpansionId,
    SourcePosition, SourcePreprocError, SourcePreprocModel, SourcePreprocUnavailable, SourceRange,
};
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use syntax::{
    Compilation, ParserExpectedSyntax, PreprocessorTrace, SourceBufferOrigin, SyntaxDiagnostic,
    SyntaxTree, SyntaxTreeBuffer, SyntaxTreeBufferIds, SyntaxTreeOptions,
};
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    path_identity::PathIdentityIndex,
};
use vfs::{FileId, VfsPath, anchored_path::AnchoredPath};

use crate::base_db::{
    compilation_plan::{self, CompilationPlan},
    diagnostics_config::{DiagnosticSource, DiagnosticsConfig},
    project::{CompilationProfileId, PreprocessConfig, ProjectConfig},
    source_root::{SourceRoot, SourceRootId},
};

pub trait FileLoader {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId>;
}

// Source code, syntax tree and project model.
// Everything else is derived from these queries.
#[salsa::query_group(SourceDbStorage)]
pub trait SourceDb: FileLoader + std::fmt::Debug {
    #[salsa::input]
    fn file_text(&self, file_id: FileId) -> Arc<str>;

    #[salsa::input]
    fn file_kind(&self, file_id: FileId) -> SourceFileKind;

    #[salsa::input]
    fn file_path(&self, file_id: FileId) -> Option<utils::paths::AbsPathBuf>;

    #[salsa::input]
    fn file_preprocess_config(&self, file_id: FileId) -> Arc<PreprocessConfig>;

    fn parse_src(&self, file_id: FileId) -> SyntaxTree;

    #[salsa::input]
    fn files(&self) -> Box<FxHashSet<FileId>>;

    #[salsa::input]
    fn diagnostics_config(&self) -> Arc<DiagnosticsConfig>;

    #[salsa::input]
    fn project_config(&self) -> Arc<ProjectConfig>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceFileKind {
    #[default]
    SystemVerilog,
    IncludeHeader,
    LibraryMap,
    ProjectManifest,
}

impl SourceFileKind {
    pub fn from_path(path: &VfsPath) -> Self {
        match path.name_and_extension() {
            Some((name, Some(ext))) if name == "vide" && ext.eq_ignore_ascii_case("toml") => {
                Self::ProjectManifest
            }
            Some((_, Some(ext))) if ext.eq_ignore_ascii_case("map") => Self::LibraryMap,
            Some((_, Some(ext)))
                if ["vh", "svh", "svi"].iter().any(|header| ext.eq_ignore_ascii_case(header)) =>
            {
                Self::IncludeHeader
            }
            _ => Self::SystemVerilog,
        }
    }

    pub(crate) fn is_semantic_compilation_unit(self) -> bool {
        matches!(self, Self::SystemVerilog | Self::LibraryMap)
    }

    fn is_slang_parse_unit(self) -> bool {
        matches!(self, Self::SystemVerilog | Self::LibraryMap)
    }
}

fn parse_src(db: &dyn SourceDb, file_id: FileId) -> SyntaxTree {
    let _span = tracing::info_span!("slang.parse_src", ?file_id).entered();
    let text = db.file_text(file_id);

    match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            // HIR source maps are local to the queried file; project-aware
            // include expansion belongs to parse_src_for_compilation.
            let preprocess = db.file_preprocess_config(file_id);
            let include_paths = preprocess.include_dir_strings();
            let options = syntax::SyntaxTreeOptions {
                predefines: preprocess.predefines.clone(),
                include_paths,
                ..syntax::SyntaxTreeOptions::without_include_expansion()
            };
            let _span = tracing::info_span!(
                "slang.syntax_tree.from_text",
                ?file_id,
                bytes = text.len(),
                include_buffer_count = 0usize
            )
            .entered();
            SyntaxTree::from_text_with_options(&text, "", "", &options)
        }
        SourceFileKind::LibraryMap => SyntaxTree::from_library_map_text(&text, "", ""),
        SourceFileKind::ProjectManifest => SyntaxTree::from_text("", "", ""),
    }
}

struct SourceFileIdentity {
    name: String,
    path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationDiagnostic {
    /// File attribution after mapping slang source buffers back to VFS files.
    pub file_id: FileId,
    /// The compilation phase that produced the diagnostic.
    pub source: DiagnosticSource,
    pub diagnostic: SyntaxDiagnostic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedSourcePreprocModel {
    pub model: SourcePreprocModel,
    pub source_map: PreprocSourceMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocSourceMap {
    entries: FxHashMap<PreprocSourceId, PreprocSourceMapping>,
    expansion_entries: FxHashMap<SourceMacroExpansionId, PreprocExpansionMapping>,
    text_lengths: FxHashMap<PreprocSourceId, usize>,
    range_offsets: FxHashMap<PreprocSourceId, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocSourceMapping {
    RealFile(FileId),
    VirtualFile { file_id: FileId, path: VfsPath, origin: PreprocVirtualOrigin },
    Unmapped(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocExpansionMapping {
    pub file_id: FileId,
    pub path: VfsPath,
    pub origin: PreprocVirtualOrigin,
    pub text: String,
    pub emitted_range: SourceEmittedTokenRange,
    token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
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
}

impl PreprocSourceMap {
    pub fn insert_real_file(&mut self, source: PreprocSourceId, file_id: FileId, text_len: usize) {
        self.entries.insert(source, PreprocSourceMapping::RealFile(file_id));
        self.text_lengths.insert(source, text_len);
        self.range_offsets.insert(source, 0);
    }

    pub fn insert_virtual_file(
        &mut self,
        source: PreprocSourceId,
        file_id: FileId,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
    ) {
        self.insert_virtual_file_with_offset(source, file_id, path, origin, text_len, 0);
    }

    fn insert_virtual_file_with_offset(
        &mut self,
        source: PreprocSourceId,
        file_id: FileId,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
        range_offset: usize,
    ) {
        self.entries.insert(source, PreprocSourceMapping::VirtualFile { file_id, path, origin });
        self.text_lengths.insert(source, text_len);
        self.range_offsets.insert(source, range_offset);
    }

    pub fn insert_unmapped(&mut self, source: PreprocSourceId, reason: SourcePreprocUnavailable) {
        self.entries.insert(source, PreprocSourceMapping::Unmapped(reason));
        self.text_lengths.remove(&source);
        self.range_offsets.remove(&source);
    }

    pub fn get(&self, source: PreprocSourceId) -> Option<&PreprocSourceMapping> {
        self.entries.get(&source)
    }

    pub fn insert_expansion_virtual_file(
        &mut self,
        expansion: SourceMacroExpansionId,
        file_id: FileId,
        path: VfsPath,
        text: String,
        emitted_range: SourceEmittedTokenRange,
        token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
    ) {
        self.expansion_entries.insert(
            expansion,
            PreprocExpansionMapping {
                file_id,
                path,
                origin: PreprocVirtualOrigin::Expansion { expansion },
                text,
                emitted_range,
                token_ranges,
            },
        );
    }

    pub fn expansion(&self, expansion: SourceMacroExpansionId) -> Option<&PreprocExpansionMapping> {
        self.expansion_entries.get(&expansion)
    }

    pub fn expansion_source(
        &self,
        expansion: SourceMacroExpansionId,
    ) -> Result<PreprocSourceMapping, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        Ok(PreprocSourceMapping::VirtualFile {
            file_id: entry.file_id,
            path: entry.path.clone(),
            origin: entry.origin.clone(),
        })
    }

    pub fn emitted_token_range(
        &self,
        expansion: SourceMacroExpansionId,
        emitted_range: SourceEmittedTokenRange,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        expansion_text_range(entry, emitted_range)
            .ok_or(PreprocSourceMapError::MissingEmittedTokenRange { range: emitted_range })
    }

    pub fn emitted_token_text_range(
        &self,
        expansion: SourceMacroExpansionId,
        token: SourceEmittedTokenId,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        entry
            .token_ranges
            .get(&token)
            .copied()
            .ok_or(PreprocSourceMapError::MissingEmittedToken { token })
    }

    pub fn file_id(&self, source: PreprocSourceId) -> Result<FileId, PreprocSourceMapError> {
        match self.get(source) {
            Some(PreprocSourceMapping::RealFile(file_id)) => Ok(*file_id),
            Some(PreprocSourceMapping::VirtualFile { file_id, .. }) => Ok(*file_id),
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
            | Some(PreprocSourceMapping::VirtualFile { .. }) => {}
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocQueryError {
    UnsupportedFileKind(SourceFileKind),
    TraceUnavailable,
    Model(SourcePreprocError),
    UnmappedSource { buffer_id: u32, path: String },
}

fn source_file_identity(db: &dyn SourceDb, file_id: FileId) -> SourceFileIdentity {
    let path = db.file_path(file_id).map(|path| path.to_string()).unwrap_or_default();
    let name = if path.is_empty() { "source".to_owned() } else { path.clone() };
    SourceFileIdentity { name, path }
}

fn path_file_ids(db: &dyn SourceRootDb) -> PathIdentityIndex<FileId> {
    let mut index = PathIdentityIndex::default();
    for file_id in db.files().iter().copied() {
        if db.file_is_project_ignored(file_id) {
            continue;
        }
        if let Some(path) = db.file_path(file_id) {
            index.insert_path(&path, file_id);
        }
    }
    index
}

fn insert_buffer_file_ids(
    buffer_file_ids: &mut FxHashMap<u32, FileId>,
    path_file_ids: &PathIdentityIndex<FileId>,
    buffers: SyntaxTreeBufferIds,
    root_file_id: FileId,
) {
    buffer_file_ids.insert(buffers.root_buffer_id, root_file_id);
    for buffer in buffers.source_buffers {
        if let Some(file_id) = path_file_ids.get(&buffer.path) {
            buffer_file_ids.insert(buffer.buffer_id, file_id);
        }
    }
}

fn source_preproc_file_ids(
    db: &dyn SourceRootDb,
    file_id: FileId,
    profile_id: Option<CompilationProfileId>,
    trace: &PreprocessorTrace,
    options: &SyntaxTreeOptions,
) -> Result<PreprocSourceMap, SourcePreprocQueryError> {
    let mut source_map = PreprocSourceMap::default();
    let path_file_ids = path_file_ids(db);
    let root_source = PreprocSourceId::from(trace.root_buffer_id);
    source_map.insert_real_file(root_source, file_id, db.file_text(file_id).len());
    let include_buffer_texts = include_buffer_texts_by_path(options);
    let predefine_sources = trace
        .source_buffers
        .iter()
        .filter(|source| source.origin == SourceBufferOrigin::Predefine)
        .map(|source| PreprocSourceId::from(source.buffer_id))
        .collect::<Vec<_>>();
    let predefine_map =
        PredefineVirtualMapping::new(db, profile_id, &options.predefines, predefine_sources);

    for source in &trace.source_buffers {
        let source_id = PreprocSourceId::from(source.buffer_id);
        if source_id == root_source {
            source_map.insert_real_file(source_id, file_id, db.file_text(file_id).len());
            continue;
        }

        match source.origin {
            SourceBufferOrigin::Source => {
                if let Some(mapped_file_id) = path_file_ids.get(&source.path) {
                    source_map.insert_real_file(
                        source_id,
                        mapped_file_id,
                        db.file_text(mapped_file_id).len(),
                    );
                    continue;
                }

                if let Some(text) = include_buffer_texts.get(&source.path) {
                    let path =
                        preproc_virtual_include_buffer_path(profile_id, source_id, &source.path);
                    let file_id = preproc_virtual_file_id(db, &path);
                    source_map.insert_virtual_file(
                        source_id,
                        file_id,
                        path,
                        PreprocVirtualOrigin::ExternalIncludeBuffer { source: source_id },
                        text.len(),
                    );
                    continue;
                }

                source_map.insert_unmapped(
                    source_id,
                    SourcePreprocUnavailable::DetachedSource { source: source_id },
                );
            }
            SourceBufferOrigin::Predefine => {
                if let Some(entry) = predefine_map.entry(source_id) {
                    source_map.insert_virtual_file_with_offset(
                        source_id,
                        entry.file_id,
                        entry.path.clone(),
                        PreprocVirtualOrigin::Predefines { profile: profile_id },
                        entry.text_len,
                        entry.range_offset,
                    );
                } else {
                    source_map.insert_unmapped(
                        source_id,
                        SourcePreprocUnavailable::DetachedSource { source: source_id },
                    );
                }
            }
        }
    }

    Ok(source_map)
}

pub fn preproc_virtual_predefines_path(profile_id: Option<CompilationProfileId>) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/predefines.sv",
        profile_path_segment(profile_id)
    ))
}

pub fn preproc_virtual_builtin_path(
    profile_id: Option<CompilationProfileId>,
    name: &str,
) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/builtin/{}.sv",
        profile_path_segment(profile_id),
        sanitize_path_segment(name)
    ))
}

pub fn preproc_virtual_expansion_path(
    profile_id: Option<CompilationProfileId>,
    expansion: SourceMacroExpansionId,
) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/expansion/{}.sv",
        profile_path_segment(profile_id),
        expansion.raw()
    ))
}

pub fn preproc_virtual_speculative_path(
    profile_id: Option<CompilationProfileId>,
    universe: PreprocSpeculativeUniverseId,
    root: &str,
) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/speculative/{}/{}.sv",
        profile_path_segment(profile_id),
        universe.0,
        sanitize_path_segment(root)
    ))
}

fn preproc_virtual_include_buffer_path(
    profile_id: Option<CompilationProfileId>,
    source_id: PreprocSourceId,
    source_path: &str,
) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/include-buffer/{}/{}.svh",
        profile_path_segment(profile_id),
        source_id.raw(),
        source_basename(source_path)
    ))
}

fn profile_path_segment(profile_id: Option<CompilationProfileId>) -> String {
    profile_id
        .map(|profile_id| format!("profile-{}", profile_id.0))
        .unwrap_or_else(|| "default".to_owned())
}

fn source_basename(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or("buffer");
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    sanitize_path_segment(stem)
}

fn sanitize_path_segment(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => out.push(ch),
            _ => out.push('_'),
        }
    }
    if out.is_empty() { "unnamed".to_owned() } else { out }
}

fn include_buffer_texts_by_path(options: &SyntaxTreeOptions) -> FxHashMap<String, String> {
    options
        .include_buffers
        .iter()
        .map(|buffer| (buffer.path.clone(), buffer.text.clone()))
        .collect()
}

fn materialized_predefine_text(predefine: &str) -> String {
    let mut definition = predefine.to_owned();
    if let Some(index) = definition.find('=') {
        definition.replace_range(index..index + 1, " ");
    } else {
        definition.push_str(" 1");
    }
    format!("`define {definition}\n")
}

struct PredefineVirtualMapping {
    entries: FxHashMap<PreprocSourceId, PredefineVirtualEntry>,
}

struct PredefineVirtualEntry {
    file_id: FileId,
    path: VfsPath,
    text_len: usize,
    range_offset: usize,
}

impl PredefineVirtualMapping {
    fn new(
        db: &dyn SourceRootDb,
        profile_id: Option<CompilationProfileId>,
        predefines: &[String],
        mut sources: Vec<PreprocSourceId>,
    ) -> Self {
        sources.sort_by_key(|source| source.raw());
        if sources.len() != predefines.len() || sources.is_empty() {
            return Self { entries: FxHashMap::default() };
        }

        let texts = predefines
            .iter()
            .map(|predefine| materialized_predefine_text(predefine))
            .collect::<Vec<_>>();
        let text_len = texts.iter().map(String::len).sum();
        let path = preproc_virtual_predefines_path(profile_id);
        let file_id = preproc_virtual_file_id(db, &path);
        let mut range_offset = 0usize;
        let mut entries = FxHashMap::default();
        for (source, text) in sources.into_iter().zip(texts) {
            entries.insert(
                source,
                PredefineVirtualEntry { file_id, path: path.clone(), text_len, range_offset },
            );
            range_offset += text.len();
        }

        Self { entries }
    }

    fn entry(&self, source: PreprocSourceId) -> Option<&PredefineVirtualEntry> {
        self.entries.get(&source)
    }
}

fn preproc_virtual_file_id(db: &dyn SourceRootDb, path: &VfsPath) -> FileId {
    file_id_for_vfs_path(db, path).unwrap_or_else(|| synthetic_virtual_file_id(path))
}

fn file_id_for_vfs_path(db: &dyn SourceRootDb, path: &VfsPath) -> Option<FileId> {
    for file_id in db.files().iter().copied() {
        let source_root_id = db.source_root_id(file_id);
        let source_root = db.source_root(source_root_id);
        if source_root.path_for_file(&file_id) == Some(path) {
            return Some(file_id);
        }
    }
    None
}

fn synthetic_virtual_file_id(path: &VfsPath) -> FileId {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    FileId(0x8000_0000 | ((hasher.finish() as u32) & 0x3fff_ffff))
}

fn shift_text_range(range: TextRange, offset: usize) -> Option<TextRange> {
    let start = usize::from(range.start()).checked_add(offset)?;
    let end = usize::from(range.end()).checked_add(offset)?;
    Some(TextRange::new(
        TextSize::from(u32::try_from(start).ok()?),
        TextSize::from(u32::try_from(end).ok()?),
    ))
}

fn unshift_text_size(offset: TextSize, range_offset: usize) -> Option<TextSize> {
    let offset = usize::from(offset).checked_sub(range_offset)?;
    Some(TextSize::from(u32::try_from(offset).ok()?))
}

fn expansion_text_range(
    entry: &PreprocExpansionMapping,
    emitted_range: SourceEmittedTokenRange,
) -> Option<TextRange> {
    if emitted_range.len == 0 {
        return Some(TextRange::empty(TextSize::from(0)));
    }

    let start = emitted_range.start;
    let end = SourceEmittedTokenId::new(start.raw().checked_add(emitted_range.len - 1)?);
    let start_range = entry.token_ranges.get(&start)?;
    let end_range = entry.token_ranges.get(&end)?;
    Some(TextRange::new(start_range.start(), end_range.end()))
}

fn materialize_expansion_virtual_files(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
    model: &SourcePreprocModel,
    source_map: &mut PreprocSourceMap,
) {
    for expansion in model.macro_expansions().iter() {
        let Some((text, token_ranges)) =
            materialized_expansion_text_and_ranges(model, expansion.emitted_token_range)
        else {
            continue;
        };
        let path = preproc_virtual_expansion_path(profile_id, expansion.id);
        let file_id = preproc_virtual_file_id(db, &path);
        source_map.insert_expansion_virtual_file(
            expansion.id,
            file_id,
            path,
            text,
            expansion.emitted_token_range,
            token_ranges,
        );
    }
}

fn materialized_expansion_text_and_ranges(
    model: &SourcePreprocModel,
    emitted_range: SourceEmittedTokenRange,
) -> Option<(String, FxHashMap<SourceEmittedTokenId, TextRange>)> {
    let mut text = String::new();
    let mut token_ranges = FxHashMap::default();

    for raw in
        emitted_range.start.raw()..emitted_range.start.raw().checked_add(emitted_range.len)?
    {
        let token_id = SourceEmittedTokenId::new(raw);
        let token = model.emitted_tokens().get(token_id)?;
        if !text.is_empty() {
            text.push(' ');
        }
        let start = text.len();
        text.push_str(token.text.as_str());
        let end = text.len();
        token_ranges.insert(
            token_id,
            TextRange::new(
                TextSize::from(u32::try_from(start).ok()?),
                TextSize::from(u32::try_from(end).ok()?),
            ),
        );
    }

    Some((text, token_ranges))
}

fn syntax_tree_options_for_file(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> syntax::SyntaxTreeOptions {
    let _span = tracing::info_span!("slang.syntax_tree_options.file", ?file_id).entered();
    let preprocess = db.file_preprocess_config(file_id);
    let profile_id = db.file_compilation_profile(file_id);
    let include_buffers = db.include_buffers_for_profile(profile_id).as_ref().clone();
    syntax::SyntaxTreeOptions {
        predefines: preprocess.predefines.clone(),
        include_paths: preprocess.include_dir_strings(),
        include_buffers,
        ..syntax::SyntaxTreeOptions::default()
    }
}

fn syntax_tree_options_for_profile(
    project_config: &ProjectConfig,
    profile_id: Option<CompilationProfileId>,
    include_buffers: Vec<SyntaxTreeBuffer>,
) -> syntax::SyntaxTreeOptions {
    let preprocess = project_config.preprocess_for_profile(profile_id);
    let include_paths = preprocess.include_dir_strings();
    syntax::SyntaxTreeOptions {
        predefines: preprocess.predefines,
        include_paths,
        include_buffers,
        ..syntax::SyntaxTreeOptions::default()
    }
}

fn parse_src_for_compilation(db: &dyn SourceRootDb, file_id: FileId) -> SyntaxTree {
    let _span = tracing::info_span!("slang.parse_for_compilation", ?file_id).entered();
    let text = {
        let _span =
            tracing::info_span!("slang.parse_for_compilation.file_text", ?file_id).entered();
        db.file_text(file_id)
    };
    let identity = source_file_identity(db, file_id);

    match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            let options = syntax_tree_options_for_file(db, file_id);
            let include_buffer_count = options.include_buffers.len();
            let _span = tracing::info_span!(
                "slang.parse_for_compilation.from_text",
                ?file_id,
                bytes = text.len(),
                include_buffer_count
            )
            .entered();
            SyntaxTree::from_text_with_options(&text, &identity.name, &identity.path, &options)
        }
        SourceFileKind::LibraryMap => {
            SyntaxTree::from_library_map_text(&text, &identity.name, &identity.path)
        }
        SourceFileKind::ProjectManifest => SyntaxTree::from_text("", "", ""),
    }
}

fn parser_expected_syntax(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> Arc<[ParserExpectedSyntax]> {
    if matches!(db.file_kind(file_id), SourceFileKind::ProjectManifest) {
        return Arc::from(Vec::<ParserExpectedSyntax>::new());
    }

    let text = db.file_text(file_id);
    let identity = source_file_identity(db, file_id);
    let offset = usize::from(offset);
    let expected = match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            let options = syntax_tree_options_for_file(db, file_id);
            SyntaxTree::expected_syntax_at_offset_with_options(
                &text,
                &identity.name,
                &identity.path,
                offset,
                &options,
            )
        }
        SourceFileKind::LibraryMap => SyntaxTree::library_map_expected_syntax_at_offset(
            &text,
            &identity.name,
            &identity.path,
            offset,
        ),
        SourceFileKind::ProjectManifest => Vec::new(),
    };
    Arc::from(expected)
}

fn parse_diagnostics(db: &dyn SourceRootDb, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
    let config = db.diagnostics_config();
    if !config.enabled || !config.parse.enabled || !db.file_kind(file_id).is_slang_parse_unit() {
        return Arc::from(Vec::<SyntaxDiagnostic>::new());
    }

    let _span = tracing::info_span!("slang.parse_diagnostics", ?file_id).entered();
    let tree = {
        let _span = tracing::info_span!("slang.parse_diagnostics.parse_tree", ?file_id).entered();
        db.parse_src_for_compilation(file_id)
    };
    let root_buffer_id = tree.buffer_id();
    let raw_diagnostics = {
        let _span = tracing::info_span!("slang.parse.raw_diagnostics", ?file_id).entered();
        tree.diagnostics_with_options(&config.slang.warnings)
    };
    let raw_diagnostic_count = raw_diagnostics.len();
    let mut non_root_buffer_count = 0usize;
    let mut ignored_diagnostic_count = 0usize;
    let mut diags = Vec::new();

    for diag in raw_diagnostics {
        if !diag.buffer_id.is_none_or(|buffer_id| buffer_id == root_buffer_id) {
            non_root_buffer_count += 1;
            continue;
        }

        match config.apply_rules(DiagnosticSource::Parse, diag) {
            Some(diag) => diags.push(diag),
            None => ignored_diagnostic_count += 1,
        }
    }

    tracing::info!(
        raw_diagnostic_count,
        non_root_buffer_count,
        ignored_diagnostic_count,
        diagnostic_count = diags.len(),
        "parse diagnostics complete"
    );
    Arc::from(diags)
}

// Don't expose source roots to HIR, so extract them in a separate DB.
#[salsa::query_group(SourceRootDbStorage)]
pub trait SourceRootDb: SourceDb {
    #[salsa::input]
    fn source_root_id(&self, file_id: FileId) -> SourceRootId;

    #[salsa::input]
    fn source_root(&self, id: SourceRootId) -> Arc<SourceRoot>;

    fn file_compilation_profile(&self, file_id: FileId) -> Option<CompilationProfileId>;
    fn file_is_project_ignored(&self, file_id: FileId) -> bool;
    fn compilation_plan_for_root(&self, source_root_id: SourceRootId) -> Arc<CompilationPlan>;
    fn compilation_plan_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<CompilationPlan>;
    /// Diagnostics produced by one slang compilation profile. This is the
    /// semantic diagnostics path, but it also returns parse diagnostics from
    /// the same syntax trees so one request does not parse the same roots
    /// twice.
    fn compilation_profile_diagnostics(
        &self,
        profile_id: CompilationProfileId,
    ) -> Arc<[CompilationDiagnostic]>;
    fn include_buffers_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<Vec<SyntaxTreeBuffer>>;
    fn source_preproc_model(
        &self,
        file_id: FileId,
    ) -> Arc<Result<MappedSourcePreprocModel, SourcePreprocQueryError>>;
    fn macro_reference_index_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<crate::preproc::MacroReferenceIndex>;
    fn parse_src_for_compilation(&self, file_id: FileId) -> SyntaxTree;
    fn parser_expected_syntax(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Arc<[ParserExpectedSyntax]>;
    fn parse_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]>;
    /// Diagnostics for the compilation profile that owns `file_id`.
    fn file_compilation_diagnostics(&self, file_id: FileId) -> Arc<[CompilationDiagnostic]>;
    fn semantic_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]>;
    fn source_root_semantic_diagnostics(
        &self,
        file_id: FileId,
    ) -> Arc<[(FileId, SyntaxDiagnostic)]>;
}

fn file_compilation_profile(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Option<CompilationProfileId> {
    let source_root_id = db.source_root_id(file_id);
    let project_config = db.project_config();
    let profile_id = project_config.profile_for_root(source_root_id);
    let source_root = db.source_root(source_root_id);
    if profile_id.is_none() && source_root.role().reports_missing_profile() {
        tracing::debug!(
            ?file_id,
            ?source_root_id,
            root_profile_count = project_config.root_profile_count(),
            "file has no compilation profile",
        );
    }
    profile_id
}

fn file_is_project_ignored(db: &dyn SourceRootDb, file_id: FileId) -> bool {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).is_ignored()
}

fn compilation_plan_for_root(
    db: &dyn SourceRootDb,
    source_root_id: SourceRootId,
) -> Arc<CompilationPlan> {
    Arc::new(CompilationPlan::for_source_root(db, source_root_id))
}

fn compilation_plan_for_profile(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<CompilationPlan> {
    Arc::new(CompilationPlan::for_profile(db, profile_id))
}

fn include_buffers_for_profile(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<Vec<SyntaxTreeBuffer>> {
    let plan = db.compilation_plan_for_profile(profile_id);
    Arc::new(compilation_plan::include_buffers_for_plan(db, &plan))
}

fn source_preproc_model(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<Result<MappedSourcePreprocModel, SourcePreprocQueryError>> {
    let file_kind = db.file_kind(file_id);
    if !matches!(file_kind, SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader) {
        return Arc::new(Err(SourcePreprocQueryError::UnsupportedFileKind(file_kind)));
    }

    let text = db.file_text(file_id);
    let identity = source_file_identity(db, file_id);
    let profile_id = db.file_compilation_profile(file_id);
    let options = syntax_tree_options_for_file(db, file_id);
    let Some(trace) =
        SyntaxTree::preprocessor_trace(&text, &identity.name, &identity.path, &options)
    else {
        return Arc::new(Err(SourcePreprocQueryError::TraceUnavailable));
    };

    let mut source_map = match source_preproc_file_ids(db, file_id, profile_id, &trace, &options) {
        Ok(source_map) => source_map,
        Err(err) => return Arc::new(Err(err)),
    };
    let model = match SourcePreprocModel::from_trace(trace) {
        Ok(model) => model,
        Err(err) => return Arc::new(Err(SourcePreprocQueryError::Model(err))),
    };
    materialize_expansion_virtual_files(db, profile_id, &model, &mut source_map);

    Arc::new(Ok(MappedSourcePreprocModel { model, source_map }))
}

fn macro_reference_index_for_profile(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<crate::preproc::MacroReferenceIndex> {
    Arc::new(crate::preproc::build_macro_reference_index(db, profile_id))
}

fn semantic_diagnostics(db: &dyn SourceRootDb, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
    Arc::from(
        db.source_root_semantic_diagnostics(file_id)
            .iter()
            .filter_map(|(diag_file_id, diag)| (*diag_file_id == file_id).then_some(diag.clone()))
            .collect::<Vec<_>>(),
    )
}

fn file_compilation_diagnostics(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<[CompilationDiagnostic]> {
    let source_root_id = db.source_root_id(file_id);
    let config = db.diagnostics_config();
    if !config.enabled || !config.semantic.enabled || db.file_is_project_ignored(file_id) {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    }

    let project_config = db.project_config();
    let Some(profile_id) = project_config.profile_for_root(source_root_id) else {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    };
    db.compilation_profile_diagnostics(profile_id)
}

fn compilation_profile_diagnostics(
    db: &dyn SourceRootDb,
    profile_id: CompilationProfileId,
) -> Arc<[CompilationDiagnostic]> {
    let config = db.diagnostics_config();
    if !config.enabled || !config.semantic.enabled {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    }

    let project_config = db.project_config();
    let plan = db.compilation_plan_for_profile(Some(profile_id));
    let compilation_include_buffers = {
        let _span = tracing::info_span!("slang.semantic.compilation_buffers").entered();
        compilation_plan::compilation_source_buffers_for_plan(db, &plan)
    };
    let root_count = plan.roots.len();
    let top_module_count = plan.top_modules.len();
    let include_buffer_count = compilation_include_buffers.len();
    let _span = tracing::info_span!(
        "slang.compilation_profile_diagnostics",
        ?profile_id,
        root_count,
        top_module_count,
        include_buffer_count
    )
    .entered();
    let compilation_options = syntax_tree_options_for_profile(
        &project_config,
        Some(profile_id),
        compilation_include_buffers,
    );
    let mut compilation = Compilation::new_with_top_modules(&plan.top_modules);
    let mut buffer_file_ids = FxHashMap::default();
    let path_file_ids = path_file_ids(db);
    let mut compilation_root_count = 0usize;
    let mut compilation_buffer_count = 0usize;
    {
        let _span = tracing::info_span!("slang.semantic.add_roots", root_count).entered();
        for file_id in plan.roots.iter().copied() {
            let text = {
                let _span =
                    tracing::info_span!("slang.semantic.add_root.file_text", ?file_id).entered();
                db.file_text(file_id)
            };
            let identity = source_file_identity(db, file_id);
            let buffer_ids = match db.file_kind(file_id) {
                SourceFileKind::SystemVerilog => {
                    let include_buffer_count = compilation_options.include_buffers.len();
                    let _span = tracing::info_span!(
                        "slang.semantic.add_root.from_text",
                        ?file_id,
                        bytes = text.len(),
                        include_buffer_count
                    )
                    .entered();
                    compilation.add_syntax_tree_from_text(
                        &text,
                        &identity.name,
                        &identity.path,
                        &compilation_options,
                    )
                }
                SourceFileKind::LibraryMap => compilation.add_library_map_syntax_tree_from_text(
                    &text,
                    &identity.name,
                    &identity.path,
                ),
                SourceFileKind::IncludeHeader | SourceFileKind::ProjectManifest => continue,
            };
            compilation_root_count += 1;
            compilation_buffer_count += 1 + buffer_ids.source_buffers.len();
            insert_buffer_file_ids(&mut buffer_file_ids, &path_file_ids, buffer_ids, file_id);
        }
    }
    tracing::info!(
        compilation_root_count,
        compilation_buffer_count,
        mapped_buffer_count = buffer_file_ids.len(),
        "semantic compilation roots added"
    );

    let mut diagnostics = Vec::new();
    if config.parse.enabled {
        let raw_diagnostics = {
            let _span = tracing::info_span!("slang.semantic.parse_diagnostics").entered();
            compilation.parse_diagnostics_with_options(&config.slang.warnings)
        };
        let raw_diagnostic_count = raw_diagnostics.len();
        let mut unmapped_buffer_count = 0usize;
        let mut ignored_diagnostic_count = 0usize;
        {
            let _span =
                tracing::info_span!("slang.semantic.map_parse_diagnostics", raw_diagnostic_count)
                    .entered();
            diagnostics.extend(raw_diagnostics.into_iter().filter_map(|diag| {
                let diag_file_id = match diag
                    .buffer_id
                    .and_then(|buffer_id| buffer_file_ids.get(&buffer_id).copied())
                {
                    Some(file_id) => file_id,
                    None => {
                        unmapped_buffer_count += 1;
                        return None;
                    }
                };
                let diag = match config.apply_rules(DiagnosticSource::Parse, diag) {
                    Some(diag) => diag,
                    None => {
                        ignored_diagnostic_count += 1;
                        return None;
                    }
                };
                Some(CompilationDiagnostic {
                    file_id: diag_file_id,
                    source: DiagnosticSource::Parse,
                    diagnostic: diag,
                })
            }));
        }
        tracing::info!(
            raw_diagnostic_count,
            unmapped_buffer_count,
            ignored_diagnostic_count,
            diagnostic_count = diagnostics.len(),
            "compilation parse diagnostics complete"
        );
    }

    let raw_semantic_diagnostics = {
        let _span = tracing::info_span!("slang.semantic.raw_diagnostics").entered();
        compilation.semantic_diagnostics_with_options(&config.slang.warnings)
    };
    let raw_semantic_diagnostic_count = raw_semantic_diagnostics.len();
    let mut unmapped_semantic_buffer_count = 0usize;
    let mut ignored_semantic_diagnostic_count = 0usize;
    {
        let _span =
            tracing::info_span!("slang.semantic.map_diagnostics", raw_semantic_diagnostic_count)
                .entered();
        diagnostics.extend(raw_semantic_diagnostics.into_iter().filter_map(|diag| {
            let diag_file_id =
                diag.buffer_id.and_then(|buffer_id| buffer_file_ids.get(&buffer_id).copied());
            let Some(diag_file_id) = diag_file_id else {
                unmapped_semantic_buffer_count += 1;
                return None;
            };
            let Some(diag) = config.apply_rules(DiagnosticSource::Semantic, diag) else {
                ignored_semantic_diagnostic_count += 1;
                return None;
            };
            Some(CompilationDiagnostic {
                file_id: diag_file_id,
                source: DiagnosticSource::Semantic,
                diagnostic: diag,
            })
        }));
    }
    tracing::info!(
        raw_semantic_diagnostic_count,
        unmapped_semantic_buffer_count,
        ignored_semantic_diagnostic_count,
        diagnostic_count = diagnostics.len(),
        "semantic diagnostics complete"
    );

    Arc::from(diagnostics)
}

fn source_root_semantic_diagnostics(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<[(FileId, SyntaxDiagnostic)]> {
    Arc::from(
        db.file_compilation_diagnostics(file_id)
            .iter()
            .filter_map(|diag| {
                (diag.source == DiagnosticSource::Semantic)
                    .then_some((diag.file_id, diag.diagnostic.clone()))
            })
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use rustc_hash::FxHashSet;
    use syntax::{SourceBufferId, SourceBufferOrigin, SyntaxTreeOptions};
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{FileSet, VfsPath};

    use super::*;
    use crate::base_db::salsa::{self, Durability};

    const TOP: FileId = FileId(0);
    const ROOT: SourceRootId = SourceRootId(0);

    #[salsa::database(SourceDbStorage, SourceRootDbStorage)]
    #[derive(Default)]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    impl salsa::Database for TestDb {}

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl FileLoader for TestDb {
        fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor_id);
            SourceRootDb::source_root(self, source_root_id).resolve_path(path)
        }
    }

    fn db_with_root_file() -> TestDb {
        let top_path = abs_path("rtl/top.v");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);
        let mut files = FxHashSet::default();
        files.insert(TOP);

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        db.set_source_root_id_with_durability(TOP, ROOT, Durability::LOW);
        db.set_file_path_with_durability(TOP, Some(top_path), Durability::LOW);
        db.set_file_kind_with_durability(TOP, SourceFileKind::SystemVerilog, Durability::LOW);
        db.set_file_text_with_durability(
            TOP,
            Arc::from("module top; endmodule\n"),
            Durability::LOW,
        );
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    #[test]
    fn include_headers_are_not_standalone_parse_diagnostic_units() {
        let kind =
            SourceFileKind::from_path(&VfsPath::new_virtual_path("/include/defs.svh".into()));

        assert_eq!(kind, SourceFileKind::IncludeHeader);
        assert!(!kind.is_slang_parse_unit());
    }

    #[test]
    fn systemverilog_sources_remain_parse_diagnostic_units() {
        let kind = SourceFileKind::from_path(&VfsPath::new_virtual_path("/rtl/top.sv".into()));

        assert_eq!(kind, SourceFileKind::SystemVerilog);
        assert!(kind.is_slang_parse_unit());
    }

    #[test]
    fn project_manifests_are_not_slang_parse_diagnostic_units() {
        let kind = SourceFileKind::from_path(&VfsPath::new_virtual_path("/root/vide.toml".into()));

        assert_eq!(kind, SourceFileKind::ProjectManifest);
        assert!(!kind.is_slang_parse_unit());
    }

    #[test]
    fn source_preproc_mapping_reports_unmapped_included_source() {
        let db = db_with_root_file();
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: abs_path("include/missing.vh").to_string(),
                    buffer_id: 2,
                    origin: SourceBufferOrigin::Source,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions::default();
        let source_map = source_preproc_file_ids(&db, TOP, None, &trace, &options).unwrap();

        assert_eq!(
            source_map.get(PreprocSourceId::from(2)),
            Some(&PreprocSourceMapping::Unmapped(SourcePreprocUnavailable::DetachedSource {
                source: PreprocSourceId::from(2),
            }))
        );
        assert!(matches!(
            source_map.file_id(PreprocSourceId::from(2)),
            Err(PreprocSourceMapError::UnmappedSource { .. })
        ));
    }

    #[test]
    fn source_preproc_mapping_materializes_predefines_as_virtual_file() {
        let db = db_with_root_file();
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    buffer_id: 2,
                    origin: SourceBufferOrigin::Predefine,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    buffer_id: 3,
                    origin: SourceBufferOrigin::Predefine,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions {
            predefines: vec!["FIRST=1".to_owned(), "SECOND".to_owned()],
            ..SyntaxTreeOptions::default()
        };

        let source_map = source_preproc_file_ids(&db, TOP, None, &trace, &options).unwrap();
        let first = PreprocSourceId::from(2);
        let second = PreprocSourceId::from(3);
        let expected_path = preproc_virtual_predefines_path(None);
        let first_text = materialized_predefine_text("FIRST=1");

        let Some(PreprocSourceMapping::VirtualFile { file_id, path, origin }) =
            source_map.get(first)
        else {
            panic!("first predefine should map to virtual file");
        };
        assert_eq!(path, &expected_path);
        assert_eq!(origin, &PreprocVirtualOrigin::Predefines { profile: None });

        assert_eq!(
            source_map.get(second),
            Some(&PreprocSourceMapping::VirtualFile {
                file_id: *file_id,
                path: expected_path,
                origin: PreprocVirtualOrigin::Predefines { profile: None },
            })
        );

        let second_range = SourceRange {
            source: second,
            range: TextRange::new(TextSize::from(0), TextSize::from(7)),
        };
        assert_eq!(
            source_map.map_range(second_range).unwrap(),
            TextRange::new(
                TextSize::from(u32::try_from(first_text.len()).unwrap()),
                TextSize::from(u32::try_from(first_text.len() + 7).unwrap()),
            )
        );
    }

    #[test]
    fn source_preproc_mapping_materializes_external_include_buffer_with_text() {
        let db = db_with_root_file();
        let external_path = "/external/generated_defs.vh".to_owned();
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: external_path.clone(),
                    buffer_id: 4,
                    origin: SourceBufferOrigin::Source,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions {
            include_buffers: vec![SyntaxTreeBuffer {
                path: external_path,
                text: "`define FROM_BUFFER 1\n".to_owned(),
            }],
            ..SyntaxTreeOptions::default()
        };

        let source_map =
            source_preproc_file_ids(&db, TOP, Some(CompilationProfileId(7)), &trace, &options)
                .unwrap();
        let source = PreprocSourceId::from(4);
        let Some(PreprocSourceMapping::VirtualFile { path, origin, .. }) = source_map.get(source)
        else {
            panic!("external include buffer should map to virtual file");
        };

        assert_eq!(
            path,
            &VfsPath::new_virtual_path(
                "/__vide/preproc/profile-7/include-buffer/4/generated_defs.svh".to_owned()
            )
        );
        assert_eq!(origin, &PreprocVirtualOrigin::ExternalIncludeBuffer { source });
        assert!(matches!(
            source_map.map_range(SourceRange {
                source,
                range: TextRange::new(TextSize::from(0), TextSize::from(128)),
            }),
            Err(PreprocSourceMapError::RangeOutOfBounds { .. })
        ));
    }

    #[test]
    fn preproc_virtual_paths_use_reserved_namespace() {
        assert_eq!(
            preproc_virtual_predefines_path(None),
            VfsPath::new_virtual_path("/__vide/preproc/default/predefines.sv".to_owned())
        );
        assert_eq!(
            preproc_virtual_builtin_path(Some(CompilationProfileId(3)), "bad/name"),
            VfsPath::new_virtual_path("/__vide/preproc/profile-3/builtin/bad_name.sv".to_owned())
        );
        assert_eq!(
            preproc_virtual_expansion_path(
                Some(CompilationProfileId(3)),
                SourceMacroExpansionId::new(9),
            ),
            VfsPath::new_virtual_path("/__vide/preproc/profile-3/expansion/9.sv".to_owned())
        );
        assert_eq!(
            preproc_virtual_speculative_path(
                Some(CompilationProfileId(3)),
                PreprocSpeculativeUniverseId(11),
                "root/top",
            ),
            VfsPath::new_virtual_path(
                "/__vide/preproc/profile-3/speculative/11/root_top.sv".to_owned()
            )
        );
    }
}
