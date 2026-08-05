use base_db::{
    analysis_snapshot::CompilationContext, source_db::SourceRootDb, source_root::SourceRootId,
};
use hir_def::{
    block::{Block, BlockId},
    checker::CheckerId,
    container::{InFileOrModule, InModule, ScopeId, SubroutineScope},
    covergroup::CovergroupId,
    db::HirDefDb,
    file::HirFile,
    module::{
        Module, ModuleId, PackageId,
        clocking::ClockingBlockId,
        generate::{GenerateBlock, GenerateBlockId},
    },
    source_map::Lowered,
    subroutine::Subroutine,
    symbol::NameScope,
};
use hir_ty::db::TyDb;
use preproc_expand::{
    db::{
        CompilationDiagnostic, CompilationProfileDiagnostics, ParsedCompilationUnit, ParsedProfile,
        PreprocDb,
    },
    file::HirFileId,
    macro_file::{ExpandResult, ExpansionInfo, MacroFileId},
};
use syntax::{ParserExpectedSyntax, SyntaxDiagnostic, SyntaxTree, SyntaxTreeBuffer};
use triomphe::Arc;
use vfs::FileId;

use crate::{
    semantic_index::{ModuleIndex, SemanticIndex},
    workspace_symbols::{SymbolIndex, WorkspaceSymbol},
};

#[salsa::db]
pub trait WorkspaceSymbolIndexDb: SourceRootDb + TyDb {}

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

    pub fn compilation_plan_for_root(
        &self,
        source_root_id: SourceRootId,
    ) -> Arc<preproc_expand::compilation_plan::CompilationPlan> {
        let db: &dyn PreprocDb = self;
        db.compilation_plan_for_root(source_root_id)
    }

    pub fn compilation_plan_for_profile(
        &self,
        profile_id: Option<base_db::project::CompilationProfileId>,
    ) -> Arc<preproc_expand::compilation_plan::CompilationPlan> {
        let db: &dyn PreprocDb = self;
        db.compilation_plan_for_profile(profile_id)
    }

    pub fn compilation_context(
        &self,
        profile_id: Option<base_db::project::CompilationProfileId>,
    ) -> Arc<CompilationContext> {
        let db: &dyn PreprocDb = self;
        db.compilation_context(profile_id)
    }

    pub fn compilation_context_for_file(&self, file_id: FileId) -> Arc<CompilationContext> {
        let db: &dyn PreprocDb = self;
        db.compilation_context_for_file(file_id)
    }

    pub fn compilation_profile_diagnostics(
        &self,
        profile_id: base_db::project::CompilationProfileId,
    ) -> Arc<CompilationProfileDiagnostics> {
        let db: &dyn PreprocDb = self;
        db.compilation_profile_diagnostics(profile_id)
    }

    pub fn include_buffers_for_profile(
        &self,
        profile_id: Option<base_db::project::CompilationProfileId>,
    ) -> Arc<Vec<SyntaxTreeBuffer>> {
        let db: &dyn PreprocDb = self;
        db.include_buffers_for_profile(profile_id)
    }

    pub fn parsed_compilation_unit(&self, file_id: FileId) -> ParsedCompilationUnit {
        let db: &dyn PreprocDb = self;
        db.parsed_compilation_unit(file_id)
    }

    pub fn parsed_profile(
        &self,
        profile_id: Option<base_db::project::CompilationProfileId>,
    ) -> Arc<ParsedProfile> {
        let db: &dyn PreprocDb = self;
        db.parsed_profile(profile_id)
    }

    pub fn parse_src_for_compilation(&self, file_id: FileId) -> SyntaxTree {
        let db: &dyn PreprocDb = self;
        db.parse_src_for_compilation(file_id)
    }

    pub fn parser_expected_syntax(
        &self,
        file_id: FileId,
        offset: utils::line_index::TextSize,
    ) -> Arc<[ParserExpectedSyntax]> {
        let db: &dyn PreprocDb = self;
        db.parser_expected_syntax(file_id, offset)
    }

    pub fn parse_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
        let db: &dyn PreprocDb = self;
        db.parse_diagnostics(file_id)
    }

    pub fn file_compilation_diagnostics(&self, file_id: FileId) -> Arc<[CompilationDiagnostic]> {
        let db: &dyn PreprocDb = self;
        db.file_compilation_diagnostics(file_id)
    }

    pub fn semantic_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
        let db: &dyn PreprocDb = self;
        db.semantic_diagnostics(file_id)
    }

    pub fn macro_expansion(&self, macro_file: MacroFileId) -> Arc<ExpandResult<ExpansionInfo>> {
        let db: &dyn PreprocDb = self;
        db.macro_expansion(macro_file)
    }

    pub fn parse(&self, file_id: HirFileId) -> SyntaxTree {
        let db: &dyn PreprocDb = self;
        db.parse(file_id)
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

    pub fn infer_expr(
        &self,
        expr: hir_def::container::InContainer<hir_def::expr::ExprId>,
    ) -> hir_ty::Type {
        let db: &dyn TyDb = self;
        db.infer_expr(expr)
    }

    pub fn infer_path_resolution(
        &self,
        res: hir_def::symbol::Resolution<hir_def::def_id::DefId>,
    ) -> hir_ty::Type {
        let db: &dyn TyDb = self;
        db.infer_path_resolution(res)
    }
}

#[salsa::tracked(returns(clone), unsafe(non_salsa_values))]
fn file_workspace_symbols(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
    _key: (),
) -> Arc<[WorkspaceSymbol]> {
    crate::workspace_symbols::file_symbols(db, file_id)
}

#[salsa::tracked(returns(clone), unsafe(non_salsa_values))]
fn source_root_symbol_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
    _key: (),
) -> Arc<SymbolIndex> {
    Arc::new(SymbolIndex::for_source_root(db, source_root_id))
}

#[salsa::tracked(returns(clone), unsafe(non_salsa_values))]
fn source_root_module_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
    _key: (),
) -> Arc<ModuleIndex> {
    Arc::new(ModuleIndex::for_source_root(db, source_root_id))
}

#[salsa::tracked(returns(clone), unsafe(non_salsa_values))]
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
