use base_db::{impl_intern_key, impl_intern_lookup, salsa, source_db::SourceDb};
use rustc_hash::FxHashMap;
use syntax::SyntaxTree;
use triomphe::Arc;

use crate::{
    container::InModule,
    file::HirFileId,
    hir_def::{
        Ident,
        block::{self, Block, BlockId, BlockLoc, BlockSourceMap},
        expr::data_ty::{BuiltinDataTy, BuiltinDataTyId},
        file::{self, FileSourceMap, HirFile},
        module::{self, Module, ModuleId, ModuleSourceMap},
        subroutine::{self, Subroutine, SubroutineId, SubroutineSourceMap},
    },
    scope::{BlockScope, ModuleScope, UnitScope},
};

pub(crate) macro impl_intern($id:ident, $loc:ident, $intern:ident, $lookup:ident) {
    impl_intern_key!($id);
    impl_intern_lookup!(InternDb, $id, $loc, $intern, $lookup);
}

#[salsa::query_group(InternDbStorage)]
pub trait InternDb: SourceDb {
    #[salsa::interned]
    fn intern_ty(&self, ty: BuiltinDataTy) -> BuiltinDataTyId;

    #[salsa::interned]
    fn intern_block(&self, block: BlockLoc) -> BlockId;
}

impl_intern!(BuiltinDataTyId, BuiltinDataTy, intern_ty, lookup_intern_ty);
impl_intern!(BlockId, BlockLoc, intern_block, lookup_intern_block);

#[salsa::query_group(HirDbStorage)]
pub trait HirDb: InternDb {
    #[salsa::transparent]
    fn parse(&self, file_id: HirFileId) -> SyntaxTree;

    #[salsa::invoke(file::hir_file_with_source_map_query)]
    fn hir_file_with_source_map(&self, file_id: HirFileId) -> (Arc<HirFile>, Arc<FileSourceMap>);

    fn hir_file(&self, file_id: HirFileId) -> Arc<HirFile>;

    #[salsa::invoke(module::module_with_source_map_query)]
    fn module_with_source_map(&self, module_id: ModuleId) -> (Arc<Module>, Arc<ModuleSourceMap>);

    fn module(&self, module_id: ModuleId) -> Arc<Module>;

    #[salsa::invoke(block::block_with_source_map_query)]
    fn block_with_source_map(&self, block_id: BlockId) -> (Arc<Block>, Arc<BlockSourceMap>);

    fn block(&self, block_id: BlockId) -> Arc<Block>;

    #[salsa::invoke(subroutine::subroutine_with_source_map_query)]
    fn subroutine_with_source_map(
        &self,
        subroutine: InModule<SubroutineId>,
    ) -> (Arc<Subroutine>, Arc<SubroutineSourceMap>);

    fn subroutine(&self, subroutine_id: InModule<SubroutineId>) -> Arc<Subroutine>;

    #[salsa::invoke(UnitScope::unit_scope_query)]
    fn unit_scope(&self) -> Arc<UnitScope>;

    #[salsa::invoke(UnitScope::file_scope_query)]
    fn file_scope(&self, file_id: HirFileId) -> Arc<UnitScope>;

    #[salsa::invoke(ModuleScope::module_scope_query)]
    fn module_scope(&self, module_id: ModuleId) -> Arc<ModuleScope>;

    #[salsa::invoke(BlockScope::block_scope_query)]
    fn block_scope(&self, block_id: BlockId) -> Arc<BlockScope>;
}

fn parse(db: &dyn HirDb, file_id: HirFileId) -> SyntaxTree {
    db.parse_src(file_id.file_id())
}

fn hir_file(db: &dyn HirDb, file_id: HirFileId) -> Arc<HirFile> {
    db.hir_file_with_source_map(file_id).0
}

fn module(db: &dyn HirDb, module_id: ModuleId) -> Arc<Module> {
    db.module_with_source_map(module_id).0
}

fn block(db: &dyn HirDb, block_id: BlockId) -> Arc<Block> {
    db.block_with_source_map(block_id).0
}

fn subroutine(db: &dyn HirDb, subroutine_id: InModule<SubroutineId>) -> Arc<Subroutine> {
    db.subroutine_with_source_map(subroutine_id).0
}
