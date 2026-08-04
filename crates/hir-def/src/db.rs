use base_db::{impl_intern_key, impl_intern_lookup, salsa};
use preproc_expand::{db::PreprocDb, file::HirFileId};
use triomphe::Arc;

use crate::{
    block::{self, Block, BlockId, BlockLoc},
    checker::CheckerId,
    container::{InFileOrModule, InModule, SubroutineScope},
    covergroup::CovergroupId,
    def_id::{DefId, Definition},
    expr::data_ty::{BuiltinDataTy, BuiltinDataTyId},
    file::{self, HirFile},
    module::{
        self, Module, ModuleId, PackageId,
        clocking::ClockingBlockId,
        generate::{self, GenerateBlock, GenerateBlockId, GenerateBlockLoc},
    },
    nameres::{self, DefMap},
    source_map::Lowered,
    subroutine::{self, Subroutine},
    symbol::{DefOrigin, DefOriginLoc, NameScope},
};

pub(crate) macro impl_intern($id:ident, $loc:ident, $intern:ident, $lookup:ident) {
    impl_intern_key!($id);
    impl_intern_lookup!(InternDb, $id, $loc, $intern, $lookup);
}

#[salsa::query_group(InternDbStorage)]
pub trait InternDb: PreprocDb {
    #[salsa::interned]
    fn intern_ty(&self, ty: BuiltinDataTy) -> BuiltinDataTyId;

    #[salsa::interned]
    fn intern_block(&self, block: BlockLoc) -> BlockId;

    #[salsa::interned]
    fn intern_generate_block(&self, generate_block: GenerateBlockLoc) -> GenerateBlockId;

    #[salsa::interned]
    fn intern_def_origin(&self, origin: DefOriginLoc) -> DefOrigin;

    #[salsa::interned]
    fn intern_def(&self, definition: Definition) -> DefId;
}

impl_intern!(BuiltinDataTyId, BuiltinDataTy, intern_ty, lookup_intern_ty);
impl_intern!(BlockId, BlockLoc, intern_block, lookup_intern_block);
impl_intern!(
    GenerateBlockId,
    GenerateBlockLoc,
    intern_generate_block,
    lookup_intern_generate_block
);
impl_intern!(DefOrigin, DefOriginLoc, intern_def_origin, lookup_intern_def_origin);
impl_intern!(DefId, Definition, intern_def, lookup_intern_def);

#[salsa::query_group(HirDefDbStorage)]
pub trait HirDefDb: InternDb {
    #[salsa::invoke(file::hir_file_with_source_map_query)]
    fn hir_file_with_source_map(&self, file_id: HirFileId) -> Arc<Lowered<HirFile>>;

    fn hir_file(&self, file_id: HirFileId) -> Arc<HirFile>;

    #[salsa::invoke(module::module_with_source_map_query)]
    fn module_with_source_map(&self, module_id: ModuleId) -> Arc<Lowered<Module>>;

    fn module(&self, module_id: ModuleId) -> Arc<Module>;

    #[salsa::invoke(block::block_with_source_map_query)]
    fn block_with_source_map(&self, block_id: BlockId) -> Arc<Lowered<Block>>;

    fn block(&self, block_id: BlockId) -> Arc<Block>;

    #[salsa::invoke(subroutine::subroutine_with_source_map_query)]
    fn subroutine_with_source_map(&self, subroutine: SubroutineScope) -> Arc<Lowered<Subroutine>>;

    fn subroutine(&self, subroutine_id: SubroutineScope) -> Arc<Subroutine>;

    #[salsa::invoke(generate::generate_block_with_source_map_query)]
    fn generate_block_with_source_map(
        &self,
        generate_block_id: GenerateBlockId,
    ) -> Arc<Lowered<GenerateBlock>>;

    fn generate_block(&self, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock>;

    #[salsa::invoke(nameres::def_map_query)]
    fn def_map(&self, file_id: HirFileId) -> Arc<DefMap>;

    #[salsa::invoke(NameScope::unit_scope_query)]
    fn unit_scope(&self) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::file_scope_query)]
    fn file_scope(&self, file_id: HirFileId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::module_scope_query)]
    fn module_scope(&self, module_id: ModuleId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::clocking_block_scope_query)]
    fn clocking_block_scope(&self, clocking_block_id: InModule<ClockingBlockId>) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::checker_scope_query)]
    fn checker_scope(&self, checker_id: InFileOrModule<CheckerId>) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::covergroup_scope_query)]
    fn covergroup_scope(&self, covergroup_id: InFileOrModule<CovergroupId>) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::generate_block_scope_query)]
    fn generate_block_scope(&self, generate_block_id: GenerateBlockId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::block_scope_query)]
    fn block_scope(&self, block_id: BlockId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::subroutine_scope_query)]
    fn subroutine_scope(&self, subroutine_id: SubroutineScope) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::package_export_signature_query)]
    fn package_export_signature(&self, package_id: PackageId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::package_export_scope_query)]
    fn package_export_scope(&self, package_id: PackageId) -> Arc<NameScope>;
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
    db.subroutine_with_source_map(subroutine_id).data()
}

fn generate_block(db: &dyn HirDefDb, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock> {
    db.generate_block_with_source_map(generate_block_id).data()
}
