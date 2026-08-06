use std::ops::Deref;

use base_db::salsa;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use triomphe::Arc;

use crate::{
    ast_id_map::{self, AstIdMap, SourceAstId},
    block::{self, Block, BlockId},
    checker::CheckerId,
    container::{InFileOrModule, InModule, ScopeId, SubroutineScope},
    covergroup::CovergroupId,
    diagnostics,
    file::{self, HirFile},
    item_tree::{self, ItemTree},
    module::{
        self, Module, ModuleId, PackageId,
        clocking::ClockingBlockId,
        generate::{self, GenerateBlock, GenerateBlockId},
    },
    nameres,
    owner::{self, OwnerId, OwnerSourceMap, OwnerTable},
    source_map::Lowered,
    source_projection::{self, SourceProjection},
    subroutine::{self, Subroutine, SubroutineBody},
    symbol::NameScope,
};

#[salsa::db]
pub trait HirDefDb: PreprocDb {}

// Salsa attaches tracked query methods to `dyn Db`; keep the lower-layer
// surface available on composed database trait objects without forwarding.
impl Deref for dyn HirDefDb {
    type Target = dyn PreprocDb;

    fn deref(&self) -> &Self::Target {
        self
    }
}

impl dyn HirDefDb + '_ {
    /// Stable per-file AST node ids in depth-first preorder; source-side
    /// identity is deliberately separate from owner identity.
    pub fn ast_id_map(&self, file_id: HirFileId) -> Arc<AstIdMap> {
        ast_id_map::ast_id_map(self, file_id, ())
    }

    /// The canonical owner enumeration of a file, in source order and
    /// independent of source projection and lowering.
    pub fn owner_table(&self, file_id: HirFileId) -> Arc<OwnerTable> {
        owner::owner_table(self, file_id, ())
    }

    /// Current source-side mapping for the file's canonical owners.
    pub fn owner_source_map(&self, file_id: HirFileId) -> Arc<OwnerSourceMap> {
        owner::owner_source_map(self, file_id, ())
    }

    /// Current AST identity for one canonical owner.
    pub fn owner_source_ast_id(&self, owner: OwnerId) -> Option<SourceAstId> {
        owner::owner_source_ast_id(self, owner)
    }

    pub fn item_tree(&self, file_id: HirFileId) -> Arc<ItemTree> {
        item_tree::item_tree(self, file_id, ())
    }

    pub fn source_projection(&self, file_id: HirFileId) -> Arc<SourceProjection> {
        source_projection::source_projection(self, file_id, ())
    }

    pub fn hir_file_with_source_map(&self, file_id: HirFileId) -> Arc<Lowered<HirFile>> {
        file::hir_file_with_source_map(self, file_id, ())
    }

    /// All lowering diagnostics of a file, flattened across every lowering
    /// owner (file, module, block, subroutine, generate block). Diagnostics
    /// reported without a root-buffer range get a display range resolved here
    /// (see [`diagnostics`](crate::diagnostics)).
    pub fn file_lowering_diagnostics(
        &self,
        file_id: HirFileId,
    ) -> Arc<[crate::source_map::LoweringDiagnostic]> {
        diagnostics::file_lowering_diagnostics(self, file_id, ())
    }

    pub fn hir_file(&self, file_id: HirFileId) -> Arc<HirFile> {
        hir_file(self, file_id)
    }

    pub fn module_with_source_map(&self, module_id: ModuleId) -> Arc<Lowered<Module>> {
        module::module_with_source_map(self, module_id, ())
    }

    pub fn module(&self, module_id: ModuleId) -> Arc<Module> {
        module(self, module_id)
    }

    pub fn block_with_source_map(&self, block_id: BlockId) -> Arc<Lowered<Block>> {
        block::block_with_source_map(self, block_id, ())
    }

    pub fn block(&self, block_id: BlockId) -> Arc<Block> {
        block(self, block_id)
    }

    pub fn subroutine_body_with_source_map(&self, owner: OwnerId) -> Arc<Lowered<SubroutineBody>> {
        subroutine::subroutine_body_with_source_map(self, owner)
    }

    pub fn subroutine(&self, subroutine_id: SubroutineScope) -> Arc<Subroutine> {
        subroutine(self, subroutine_id)
    }

    pub fn generate_block_with_source_map(
        &self,
        generate_block_id: GenerateBlockId,
    ) -> Arc<Lowered<GenerateBlock>> {
        generate::generate_block_with_source_map(self, generate_block_id, ())
    }

    pub fn generate_block(&self, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock> {
        generate_block(self, generate_block_id)
    }

    pub fn scope_for(&self, scope_id: ScopeId) -> Arc<NameScope> {
        nameres::scope_for(self, scope_id, ())
    }

    pub fn unit_scope(&self) -> Arc<NameScope> {
        NameScope::unit_scope(self)
    }

    pub fn file_scope(&self, file_id: HirFileId) -> Arc<NameScope> {
        NameScope::file_scope(self, file_id)
    }

    pub fn module_scope(&self, module_id: ModuleId) -> Arc<NameScope> {
        NameScope::module_scope(self, module_id)
    }

    pub fn clocking_block_scope(
        &self,
        clocking_block_id: InModule<ClockingBlockId>,
    ) -> Arc<NameScope> {
        NameScope::clocking_block_scope(self, clocking_block_id)
    }

    pub fn checker_scope(&self, checker_id: InFileOrModule<CheckerId>) -> Arc<NameScope> {
        NameScope::checker_scope(self, checker_id)
    }

    pub fn covergroup_scope(&self, covergroup_id: InFileOrModule<CovergroupId>) -> Arc<NameScope> {
        NameScope::covergroup_scope(self, covergroup_id)
    }

    pub fn generate_block_scope(&self, generate_block_id: GenerateBlockId) -> Arc<NameScope> {
        NameScope::generate_block_scope(self, generate_block_id)
    }

    pub fn block_scope(&self, block_id: BlockId) -> Arc<NameScope> {
        NameScope::block_scope(self, block_id)
    }

    pub fn subroutine_scope(&self, subroutine_id: SubroutineScope) -> Arc<NameScope> {
        NameScope::subroutine_scope(self, subroutine_id)
    }

    pub fn package_export_signature(&self, package_id: PackageId) -> Arc<NameScope> {
        NameScope::package_export_signature(self, package_id, ())
    }

    pub fn package_export_scope(&self, package_id: PackageId) -> Arc<NameScope> {
        NameScope::package_export_scope(self, package_id, ())
    }
}

fn hir_file(db: &dyn HirDefDb, file_id: HirFileId) -> Arc<HirFile> {
    db.hir_file_with_source_map(file_id).data()
}

fn module(db: &dyn HirDefDb, module_id: ModuleId) -> Arc<Module> {
    db.module_with_source_map(module_id).data()
}

fn block(db: &dyn HirDefDb, block_id: BlockId) -> Arc<Block> {
    db.block_with_source_map(block_id).data()
}

fn subroutine(db: &dyn HirDefDb, subroutine_id: SubroutineScope) -> Arc<Subroutine> {
    match subroutine_id.cont_id {
        crate::container::SubroutineParent::File(file_id) => {
            Arc::new(db.hir_file(file_id).subroutines[subroutine_id.value].clone())
        }
        crate::container::SubroutineParent::Module(module_id) => {
            Arc::new(db.module(module_id).subroutines[subroutine_id.value].clone())
        }
        crate::container::SubroutineParent::GenerateBlock(generate_block_id) => {
            Arc::new(db.generate_block(generate_block_id).subroutines[subroutine_id.value].clone())
        }
    }
}

fn generate_block(db: &dyn HirDefDb, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock> {
    db.generate_block_with_source_map(generate_block_id).data()
}

/// Sets the LRU capacity of the tracked HIR queries, mirroring the previous
/// `RootDb::update_parse_query_lru_capacity` knob.
pub fn set_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    ast_id_map::set_ast_id_map_lru_capacity(db, capacity);
    block::set_block_lru_capacity(db, capacity);
    file::set_hir_file_lru_capacity(db, capacity);
    item_tree::set_item_tree_lru_capacity(db, capacity);
    owner::set_owner_table_lru_capacity(db, capacity);
    module::set_module_lru_capacity(db, capacity);
    module::generate::set_generate_block_lru_capacity(db, capacity);
    nameres::set_scope_lru_capacity(db, capacity);
    source_projection::set_source_projection_lru_capacity(db, capacity);
    subroutine::set_subroutine_lru_capacity(db, capacity);
}
