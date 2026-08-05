use hir_def::{
    block::{Block, BlockId},
    checker::CheckerId,
    container::{InContainer, InFileOrModule, InModule, ScopeId, SubroutineScope},
    covergroup::CovergroupId,
    db::HirDefDb,
    def_id::DefId,
    expr::ExprId,
    file::{HirFile, HirFileId},
    module::{
        Module, ModuleId, PackageId,
        clocking::ClockingBlockId,
        generate::{GenerateBlock, GenerateBlockId},
    },
    source_map::Lowered,
    subroutine::Subroutine,
    symbol::{NameScope, Resolution},
};
use triomphe::Arc;

use crate::Type;

#[salsa::db]
pub trait TyDb: HirDefDb {}

impl dyn TyDb + '_ {
    pub fn infer_expr(&self, expr: InContainer<ExprId>) -> Type {
        crate::infer::type_of_expr_query(self, expr, ())
    }

    pub fn infer_path_resolution(&self, res: Resolution<DefId>) -> Type {
        crate::infer::type_of_path_resolution_query(self, res, ())
    }

    pub fn hir_file_with_source_map(&self, file_id: HirFileId) -> Arc<Lowered<HirFile>> {
        let db: &dyn HirDefDb = self;
        db.hir_file_with_source_map(file_id)
    }

    pub fn hir_file(&self, file_id: HirFileId) -> Arc<HirFile> {
        let db: &dyn HirDefDb = self;
        db.hir_file(file_id)
    }

    pub fn module_with_source_map(&self, module_id: ModuleId) -> Arc<Lowered<Module>> {
        let db: &dyn HirDefDb = self;
        db.module_with_source_map(module_id)
    }

    pub fn module(&self, module_id: ModuleId) -> Arc<Module> {
        let db: &dyn HirDefDb = self;
        db.module(module_id)
    }

    pub fn block_with_source_map(&self, block_id: BlockId) -> Arc<Lowered<Block>> {
        let db: &dyn HirDefDb = self;
        db.block_with_source_map(block_id)
    }

    pub fn block(&self, block_id: BlockId) -> Arc<Block> {
        let db: &dyn HirDefDb = self;
        db.block(block_id)
    }

    pub fn subroutine_with_source_map(
        &self,
        subroutine_id: SubroutineScope,
    ) -> Arc<Lowered<Subroutine>> {
        let db: &dyn HirDefDb = self;
        db.subroutine_with_source_map(subroutine_id)
    }

    pub fn subroutine(&self, subroutine_id: SubroutineScope) -> Arc<Subroutine> {
        let db: &dyn HirDefDb = self;
        db.subroutine(subroutine_id)
    }

    pub fn generate_block_with_source_map(
        &self,
        generate_block_id: GenerateBlockId,
    ) -> Arc<Lowered<GenerateBlock>> {
        let db: &dyn HirDefDb = self;
        db.generate_block_with_source_map(generate_block_id)
    }

    pub fn generate_block(&self, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock> {
        let db: &dyn HirDefDb = self;
        db.generate_block(generate_block_id)
    }

    pub fn scope_for(&self, scope_id: ScopeId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.scope_for(scope_id)
    }

    pub fn unit_scope(&self) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.unit_scope()
    }

    pub fn file_scope(&self, file_id: HirFileId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.file_scope(file_id)
    }

    pub fn module_scope(&self, module_id: ModuleId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.module_scope(module_id)
    }

    pub fn clocking_block_scope(
        &self,
        clocking_block_id: InModule<ClockingBlockId>,
    ) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.clocking_block_scope(clocking_block_id)
    }

    pub fn checker_scope(&self, checker_id: InFileOrModule<CheckerId>) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.checker_scope(checker_id)
    }

    pub fn covergroup_scope(&self, covergroup_id: InFileOrModule<CovergroupId>) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.covergroup_scope(covergroup_id)
    }

    pub fn generate_block_scope(&self, generate_block_id: GenerateBlockId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.generate_block_scope(generate_block_id)
    }

    pub fn block_scope(&self, block_id: BlockId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.block_scope(block_id)
    }

    pub fn subroutine_scope(&self, subroutine_id: SubroutineScope) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.subroutine_scope(subroutine_id)
    }

    pub fn package_export_signature(&self, package_id: PackageId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.package_export_signature(package_id)
    }

    pub fn package_export_scope(&self, package_id: PackageId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.package_export_scope(package_id)
    }

    pub fn parse(&self, file_id: HirFileId) -> syntax::SyntaxTree {
        let db: &dyn HirDefDb = self;
        db.parse(file_id)
    }
}
