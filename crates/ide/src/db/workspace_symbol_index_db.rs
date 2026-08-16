use std::ops::Deref;

use base_db::{source_db::SourceRootDb, source_root::SourceRootId};
use hir_ty::db::TyDb;
use triomphe::Arc;
use vfs::FileId;

use crate::{
    analysis::AnalysisContext,
    db::{SourceFileQueryKey, SourceRootQueryKey},
    semantic_index::{
        FileModuleEdges, FileModuleIndex, FileSemanticIndex, ModuleIndex, ReferenceIndex,
    },
    workspace_symbols::{SymbolIndex, WorkspaceSymbol},
};

#[salsa::db]
pub trait WorkspaceSymbolIndexDb: SourceRootDb + TyDb {}

// Expose the lower Salsa query surface without rebuilding it as IDE wrappers.
impl Deref for dyn WorkspaceSymbolIndexDb {
    type Target = dyn TyDb;

    fn deref(&self) -> &Self::Target {
        self
    }
}

impl dyn WorkspaceSymbolIndexDb + '_ {
    pub fn file_workspace_symbols(&self, file_id: FileId) -> Arc<[WorkspaceSymbol]> {
        file_workspace_symbols(self, file_id)
    }

    pub fn source_root_symbol_index(&self, source_root_id: SourceRootId) -> Arc<SymbolIndex> {
        source_root_symbol_index(self, SourceRootQueryKey::new(self, source_root_id))
    }

    pub fn source_root_module_index(&self, source_root_id: SourceRootId) -> Arc<ModuleIndex> {
        source_root_module_index(self, SourceRootQueryKey::new(self, source_root_id))
    }

    pub fn file_module_index(&self, file_id: FileId) -> Arc<FileModuleIndex> {
        file_module_index(self, file_id)
    }

    pub fn file_module_edges(&self, file_id: FileId) -> Arc<FileModuleEdges> {
        file_module_edges(self, SourceFileQueryKey::new(self, file_id))
    }

    pub fn file_semantic_index(&self, file_id: FileId) -> Arc<FileSemanticIndex> {
        file_semantic_index(self, SourceFileQueryKey::new(self, file_id))
    }

    /// Distinct source roots derived from the current file set, in stable
    /// order. Module-name resolution scans every root's module index, so both
    /// callers (`module_candidates`, `module_edges`) share one implementation
    /// instead of each recomputing `files().map(source_root_id)` inline. The
    /// per-root module/semantic indices are themselves salsa-memoized, so the
    /// only per-call work here is the cheap O(files) root-list derivation.
    pub fn workspace_source_root_ids(&self) -> Vec<SourceRootId> {
        let mut ids =
            self.files().iter().map(|&file_id| self.source_root_id(file_id)).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

}

fn file_workspace_symbols(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
) -> Arc<[WorkspaceSymbol]> {
    crate::workspace_symbols::file_symbols(db, file_id)
}

#[salsa::tracked(returns(clone))]
fn source_root_symbol_index(
    db: &dyn WorkspaceSymbolIndexDb,
    key: SourceRootQueryKey,
) -> Arc<SymbolIndex> {
    let source_root_id = key.source_root_id(db);
    Arc::new(SymbolIndex::for_source_root(db, source_root_id))
}

#[salsa::tracked(returns(clone))]
fn source_root_module_index(
    db: &dyn WorkspaceSymbolIndexDb,
    key: SourceRootQueryKey,
) -> Arc<ModuleIndex> {
    let source_root_id = key.source_root_id(db);
    Arc::new(ModuleIndex::for_source_root(db, source_root_id))
}

pub(crate) fn source_root_symbol_index_for_root(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<SymbolIndex> {
    db.source_root_symbol_index(source_root_id)
}

pub(crate) fn source_root_module_index_for_root(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<ModuleIndex> {
    db.source_root_module_index(source_root_id)
}

pub(crate) fn source_root_reference_index_for_root(
    db: &AnalysisContext<'_>,
    source_root_id: SourceRootId,
) -> Arc<ReferenceIndex> {
    db.reference_index_for_root(source_root_id)
}

fn file_module_index(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Arc<FileModuleIndex> {
    Arc::new(crate::semantic_index::FileModuleIndex::for_file(db, file_id))
}

#[salsa::tracked(returns(clone))]
fn file_module_edges(
    db: &dyn WorkspaceSymbolIndexDb,
    key: SourceFileQueryKey,
) -> Arc<FileModuleEdges> {
    let file_id = key.file_id(db);
    Arc::new(crate::semantic_index::FileModuleEdges::for_file(db, file_id))
}

#[salsa::tracked(returns(clone))]
fn file_semantic_index(
    db: &dyn WorkspaceSymbolIndexDb,
    key: SourceFileQueryKey,
) -> Arc<FileSemanticIndex> {
    let file_id = key.file_id(db);
    Arc::new(crate::semantic_index::FileSemanticIndex::for_file(db, file_id))
}

