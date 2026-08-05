use std::ops::Deref;

use base_db::{source_db::SourceRootDb, source_root::SourceRootId};
use hir_def::def_id::DefId;
use hir_ty::db::TyDb;
use triomphe::Arc;
use vfs::FileId;

use crate::{
    ScopeVisibility,
    semantic_index::{
        FileModuleEdges, FileModuleIndex, FileSemanticIndex, ModuleIndex, SemanticIndex,
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
        file_workspace_symbols(self, file_id, ())
    }

    pub fn source_root_symbol_index(&self, source_root_id: SourceRootId) -> Arc<SymbolIndex> {
        source_root_symbol_index(self, source_root_id, ())
    }

    pub fn source_root_module_index(&self, source_root_id: SourceRootId) -> Arc<ModuleIndex> {
        source_root_module_index(self, source_root_id, ())
    }

    pub fn source_root_semantic_index(&self, source_root_id: SourceRootId) -> Arc<SemanticIndex> {
        source_root_semantic_index(self, source_root_id, ())
    }

    pub fn file_module_index(&self, file_id: FileId) -> Arc<FileModuleIndex> {
        file_module_index(self, file_id, ())
    }

    pub fn file_module_edges(&self, file_id: FileId) -> Arc<FileModuleEdges> {
        file_module_edges(self, file_id, ())
    }

    pub fn file_semantic_index(&self, file_id: FileId) -> Arc<FileSemanticIndex> {
        file_semantic_index(self, file_id, ())
    }

    /// The connected component of same-name port connections around `def`,
    /// in discovery order. Shared by the recursive rename info, conflict and
    /// edit commands so a single F2 interaction computes it once.
    pub fn recursive_rename_closure(
        &self,
        def: DefId,
        visibility: ScopeVisibility,
        single_file: Option<FileId>,
    ) -> Arc<Vec<DefId>> {
        recursive_rename_closure(self, def, visibility, single_file, ())
    }
}

#[salsa::tracked(returns(clone))]
fn file_workspace_symbols(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
    _key: (),
) -> Arc<[WorkspaceSymbol]> {
    crate::workspace_symbols::file_symbols(db, file_id)
}

#[salsa::tracked(returns(clone))]
fn source_root_symbol_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
    _key: (),
) -> Arc<SymbolIndex> {
    Arc::new(SymbolIndex::for_source_root(db, source_root_id))
}

#[salsa::tracked(returns(clone))]
fn source_root_module_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
    _key: (),
) -> Arc<ModuleIndex> {
    Arc::new(ModuleIndex::for_source_root(db, source_root_id))
}

#[salsa::tracked(returns(clone))]
fn source_root_semantic_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
    _key: (),
) -> Arc<SemanticIndex> {
    Arc::new(SemanticIndex::for_source_root(db, source_root_id))
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

pub(crate) fn source_root_semantic_index_for_root(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<SemanticIndex> {
    db.source_root_semantic_index(source_root_id)
}

#[salsa::tracked(returns(clone))]
fn file_module_index(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
    _key: (),
) -> Arc<FileModuleIndex> {
    Arc::new(crate::semantic_index::FileModuleIndex::for_file(db, file_id))
}

#[salsa::tracked(returns(clone))]
fn file_module_edges(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
    _key: (),
) -> Arc<FileModuleEdges> {
    Arc::new(crate::semantic_index::FileModuleEdges::for_file(db, file_id))
}

#[salsa::tracked(returns(clone))]
fn file_semantic_index(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
    _key: (),
) -> Arc<FileSemanticIndex> {
    Arc::new(crate::semantic_index::FileSemanticIndex::for_file(db, file_id))
}

#[salsa::tracked(returns(clone))]
fn recursive_rename_closure(
    db: &dyn WorkspaceSymbolIndexDb,
    def: DefId,
    visibility: ScopeVisibility,
    single_file: Option<FileId>,
    _key: (),
) -> Arc<Vec<DefId>> {
    Arc::new(crate::rename::recursive_rename_closure_impl(db, def, visibility, single_file))
}
