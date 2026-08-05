use base_db::salsa;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use triomphe::Arc;

use crate::{
    block::{self, Block, BlockId},
    checker::CheckerId,
    container::{InFileOrModule, InModule, ScopeId, SubroutineScope},
    covergroup::CovergroupId,
    file::{self, HirFile},
    module::{
        self, Module, ModuleId, PackageId,
        clocking::ClockingBlockId,
        generate::{self, GenerateBlock, GenerateBlockId},
    },
    nameres,
    source_map::Lowered,
    subroutine::{self, Subroutine},
    symbol::NameScope,
};

#[salsa::db]
pub trait HirDefDb: PreprocDb {}

impl dyn HirDefDb + '_ {
    pub fn hir_file_with_source_map(&self, file_id: HirFileId) -> Arc<Lowered<HirFile>> {
        file::hir_file_with_source_map(self, file_id, ())
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

    pub fn subroutine_with_source_map(
        &self,
        subroutine_id: SubroutineScope,
    ) -> Arc<Lowered<Subroutine>> {
        subroutine::subroutine_with_source_map(self, subroutine_id, ())
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
        NameScope::file_scope(self, file_id, ())
    }

    pub fn module_scope(&self, module_id: ModuleId) -> Arc<NameScope> {
        NameScope::module_scope(self, module_id, ())
    }

    pub fn clocking_block_scope(
        &self,
        clocking_block_id: InModule<ClockingBlockId>,
    ) -> Arc<NameScope> {
        NameScope::clocking_block_scope(self, clocking_block_id, ())
    }

    pub fn checker_scope(&self, checker_id: InFileOrModule<CheckerId>) -> Arc<NameScope> {
        NameScope::checker_scope(self, checker_id, ())
    }

    pub fn covergroup_scope(&self, covergroup_id: InFileOrModule<CovergroupId>) -> Arc<NameScope> {
        NameScope::covergroup_scope(self, covergroup_id, ())
    }

    pub fn generate_block_scope(&self, generate_block_id: GenerateBlockId) -> Arc<NameScope> {
        NameScope::generate_block_scope(self, generate_block_id, ())
    }

    pub fn block_scope(&self, block_id: BlockId) -> Arc<NameScope> {
        NameScope::block_scope(self, block_id, ())
    }

    pub fn subroutine_scope(&self, subroutine_id: SubroutineScope) -> Arc<NameScope> {
        NameScope::subroutine_scope(self, subroutine_id, ())
    }

    pub fn package_export_signature(&self, package_id: PackageId) -> Arc<NameScope> {
        NameScope::package_export_signature(self, package_id, ())
    }

    pub fn package_export_scope(&self, package_id: PackageId) -> Arc<NameScope> {
        NameScope::package_export_scope(self, package_id, ())
    }

    pub fn parse(&self, file_id: HirFileId) -> syntax::SyntaxTree {
        let db: &dyn PreprocDb = self;
        db.parse(file_id)
    }
}
pub trait HirDefDbExt {
    fn hir_file_with_source_map(&self, file_id: HirFileId) -> Arc<Lowered<HirFile>>;
    fn hir_file(&self, file_id: HirFileId) -> Arc<HirFile>;
    fn module_with_source_map(&self, module_id: ModuleId) -> Arc<Lowered<Module>>;
    fn module(&self, module_id: ModuleId) -> Arc<Module>;
    fn block_with_source_map(&self, block_id: BlockId) -> Arc<Lowered<Block>>;
    fn block(&self, block_id: BlockId) -> Arc<Block>;
    fn subroutine_with_source_map(
        &self,
        subroutine_id: SubroutineScope,
    ) -> Arc<Lowered<Subroutine>>;
    fn subroutine(&self, subroutine_id: SubroutineScope) -> Arc<Subroutine>;
    fn generate_block_with_source_map(
        &self,
        generate_block_id: GenerateBlockId,
    ) -> Arc<Lowered<GenerateBlock>>;
    fn generate_block(&self, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock>;
    fn scope_for(&self, scope_id: ScopeId) -> Arc<NameScope>;
    fn unit_scope(&self) -> Arc<NameScope>;
    fn file_scope(&self, file_id: HirFileId) -> Arc<NameScope>;
    fn module_scope(&self, module_id: ModuleId) -> Arc<NameScope>;
    fn clocking_block_scope(&self, clocking_block_id: InModule<ClockingBlockId>) -> Arc<NameScope>;
    fn checker_scope(&self, checker_id: InFileOrModule<CheckerId>) -> Arc<NameScope>;
    fn covergroup_scope(&self, covergroup_id: InFileOrModule<CovergroupId>) -> Arc<NameScope>;
    fn generate_block_scope(&self, generate_block_id: GenerateBlockId) -> Arc<NameScope>;
    fn block_scope(&self, block_id: BlockId) -> Arc<NameScope>;
    fn subroutine_scope(&self, subroutine_id: SubroutineScope) -> Arc<NameScope>;
    fn package_export_signature(&self, package_id: PackageId) -> Arc<NameScope>;
    fn package_export_scope(&self, package_id: PackageId) -> Arc<NameScope>;
    fn parse(&self, file_id: HirFileId) -> syntax::SyntaxTree;
}

impl<Db: HirDefDb> HirDefDbExt for Db {
    fn hir_file_with_source_map(&self, file_id: HirFileId) -> Arc<Lowered<HirFile>> {
        let db: &dyn HirDefDb = self;
        db.hir_file_with_source_map(file_id)
    }

    fn hir_file(&self, file_id: HirFileId) -> Arc<HirFile> {
        let db: &dyn HirDefDb = self;
        db.hir_file(file_id)
    }

    fn module_with_source_map(&self, module_id: ModuleId) -> Arc<Lowered<Module>> {
        let db: &dyn HirDefDb = self;
        db.module_with_source_map(module_id)
    }

    fn module(&self, module_id: ModuleId) -> Arc<Module> {
        let db: &dyn HirDefDb = self;
        db.module(module_id)
    }

    fn block_with_source_map(&self, block_id: BlockId) -> Arc<Lowered<Block>> {
        let db: &dyn HirDefDb = self;
        db.block_with_source_map(block_id)
    }

    fn block(&self, block_id: BlockId) -> Arc<Block> {
        let db: &dyn HirDefDb = self;
        db.block(block_id)
    }

    fn subroutine_with_source_map(
        &self,
        subroutine_id: SubroutineScope,
    ) -> Arc<Lowered<Subroutine>> {
        let db: &dyn HirDefDb = self;
        db.subroutine_with_source_map(subroutine_id)
    }

    fn subroutine(&self, subroutine_id: SubroutineScope) -> Arc<Subroutine> {
        let db: &dyn HirDefDb = self;
        db.subroutine(subroutine_id)
    }

    fn generate_block_with_source_map(
        &self,
        generate_block_id: GenerateBlockId,
    ) -> Arc<Lowered<GenerateBlock>> {
        let db: &dyn HirDefDb = self;
        db.generate_block_with_source_map(generate_block_id)
    }

    fn generate_block(&self, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock> {
        let db: &dyn HirDefDb = self;
        db.generate_block(generate_block_id)
    }

    fn scope_for(&self, scope_id: ScopeId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.scope_for(scope_id)
    }

    fn unit_scope(&self) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.unit_scope()
    }

    fn file_scope(&self, file_id: HirFileId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.file_scope(file_id)
    }

    fn module_scope(&self, module_id: ModuleId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.module_scope(module_id)
    }

    fn clocking_block_scope(&self, clocking_block_id: InModule<ClockingBlockId>) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.clocking_block_scope(clocking_block_id)
    }

    fn checker_scope(&self, checker_id: InFileOrModule<CheckerId>) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.checker_scope(checker_id)
    }

    fn covergroup_scope(&self, covergroup_id: InFileOrModule<CovergroupId>) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.covergroup_scope(covergroup_id)
    }

    fn generate_block_scope(&self, generate_block_id: GenerateBlockId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.generate_block_scope(generate_block_id)
    }

    fn block_scope(&self, block_id: BlockId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.block_scope(block_id)
    }

    fn subroutine_scope(&self, subroutine_id: SubroutineScope) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.subroutine_scope(subroutine_id)
    }

    fn package_export_signature(&self, package_id: PackageId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.package_export_signature(package_id)
    }

    fn package_export_scope(&self, package_id: PackageId) -> Arc<NameScope> {
        let db: &dyn HirDefDb = self;
        db.package_export_scope(package_id)
    }

    fn parse(&self, file_id: HirFileId) -> syntax::SyntaxTree {
        let db: &dyn HirDefDb = self;
        db.parse(file_id)
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
    db.subroutine_with_source_map(subroutine_id).data()
}

fn generate_block(db: &dyn HirDefDb, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock> {
    db.generate_block_with_source_map(generate_block_id).data()
}
