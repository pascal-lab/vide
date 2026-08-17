//! Syntactic name occurrence table.
//!
//! The workspace product for find-references is "which files mention this
//! identifier text", not "every identifier resolved to a `DefId`". Resolution
//! happens on demand, only for occurrences of the name being searched.

use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use syntax::ptr::SyntaxTokenPtr;
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::analysis::AnalysisContext;

mod build;

/// One name-like token in a file, recorded without resolving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NameOccurrence {
    pub range: TextRange,
    pub ptr: SyntaxTokenPtr,
    pub special: bool,
}

/// Per-file slice: identifier text to the tokens that spell it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileNameIndex {
    occurrences: FxHashMap<SmolStr, Box<[NameOccurrence]>>,
}

impl FileNameIndex {
    pub(crate) fn for_file(
        db: &dyn crate::db::workspace_symbol_index_db::WorkspaceSymbolIndexDb,
        file_id: FileId,
    ) -> Self {
        build::collect_file(db, file_id)
    }

    pub(crate) fn occurrences(&self, name: &str) -> &[NameOccurrence] {
        self.occurrences.get(name).map_or(&[], |occurrences| occurrences.as_ref())
    }

    fn names(&self) -> impl Iterator<Item = &SmolStr> {
        self.occurrences.keys()
    }
}

/// Merged name → files map for one source root, plus the per-file tables.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameIndex {
    files_by_name: FxHashMap<SmolStr, Box<[FileId]>>,
    files: FxHashMap<FileId, Arc<FileNameIndex>>,
}

impl NameIndex {
    pub(crate) fn from_file_indexes(file_indexes: &FxHashMap<FileId, Arc<FileNameIndex>>) -> Self {
        let mut files_by_name: FxHashMap<SmolStr, Vec<FileId>> = FxHashMap::default();
        for (&file_id, index) in file_indexes {
            for name in index.names() {
                files_by_name.entry(name.clone()).or_default().push(file_id);
            }
        }
        for files in files_by_name.values_mut() {
            files.sort_by_key(|file_id| file_id.index());
            files.dedup();
        }
        Self {
            files_by_name: files_by_name
                .into_iter()
                .map(|(name, files)| (name, files.into_boxed_slice()))
                .collect(),
            files: file_indexes.clone(),
        }
    }

    pub(crate) fn files_mentioning(&self, name: &str) -> &[FileId] {
        self.files_by_name.get(name).map_or(&[], |files| files.as_ref())
    }
}

/// Compilation-unit files that belong in the name table for `source_root_id`.
///
/// This is the `vide.toml` / profile source set (`CompilationPlan::roots`),
/// not every path in the VFS source root.
pub(crate) fn index_files_for_root(
    ctx: &AnalysisContext<'_>,
    source_root_id: base_db::source_root::SourceRootId,
) -> Vec<FileId> {
    let plan = ctx.compilation_plan_for_root(source_root_id);
    let mut files: Vec<FileId> = plan
        .roots
        .iter()
        .copied()
        .filter(|&file_id| ctx.source_root_id(file_id) == source_root_id)
        .collect();
    files.sort_by_key(|file_id| file_id.index());
    files.dedup();
    files
}
