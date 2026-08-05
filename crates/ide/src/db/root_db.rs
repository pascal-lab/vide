use std::fmt;

use base_db::{
    analysis_snapshot::CompilationContext,
    diagnostics_config::DiagnosticsConfig,
    project::{CompilationProfileId, ProjectConfig},
    salsa::{self, Durability},
    source_db::{FileLoader, SourceDb, SourceRootDb},
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
use utils::line_index::{LineIndex, TextSize};
use vfs::{AnchoredPath, FileId};

use crate::db::{line_index_db::LineIndexDb, workspace_symbol_index_db::WorkspaceSymbolIndexDb};

#[salsa::db]
#[derive(Clone)]
pub struct RootDb {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for RootDb {}

#[salsa::db]
impl SourceDb for RootDb {}

#[salsa::db]
impl SourceRootDb for RootDb {}

#[salsa::db]
impl PreprocDb for RootDb {}

#[salsa::db]
impl HirDefDb for RootDb {}

#[salsa::db]
impl TyDb for RootDb {}

#[salsa::db]
impl LineIndexDb for RootDb {}

#[salsa::db]
impl WorkspaceSymbolIndexDb for RootDb {}

impl fmt::Debug for RootDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootDb").finish()
    }
}

impl FileLoader for RootDb {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        let source_root_id = SourceRootDb::source_root_id(self, path.anchor);
        let source_root = SourceRootDb::source_root(self, source_root_id);
        source_root.resolve_path(path)
    }
}

impl RootDb {
    pub fn new() -> RootDb {
        let mut db = RootDb { storage: salsa::Storage::default() };
        db.set_files_with_durability(Default::default(), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::HIGH);
        db
    }
}
impl RootDb {
    pub fn line_index(&self, file_id: FileId) -> Arc<LineIndex> {
        let db: &dyn LineIndexDb = self;
        db.line_index(file_id)
    }

    pub fn compilation_plan_for_root(
        &self,
        source_root_id: base_db::source_root::SourceRootId,
    ) -> Arc<preproc_expand::compilation_plan::CompilationPlan> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.compilation_plan_for_root(source_root_id)
    }

    pub fn compilation_plan_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<preproc_expand::compilation_plan::CompilationPlan> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.compilation_plan_for_profile(profile_id)
    }

    pub fn compilation_context(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<CompilationContext> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.compilation_context(profile_id)
    }

    pub fn compilation_profile_diagnostics(
        &self,
        profile_id: CompilationProfileId,
    ) -> Arc<CompilationProfileDiagnostics> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.compilation_profile_diagnostics(profile_id)
    }

    pub fn parsed_compilation_unit(&self, file_id: FileId) -> ParsedCompilationUnit {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.parsed_compilation_unit(file_id)
    }

    pub fn parsed_profile(&self, profile_id: Option<CompilationProfileId>) -> Arc<ParsedProfile> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.parsed_profile(profile_id)
    }

    pub fn parse_src_for_compilation(&self, file_id: FileId) -> SyntaxTree {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.parse_src_for_compilation(file_id)
    }

    pub fn parser_expected_syntax(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Arc<[ParserExpectedSyntax]> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.parser_expected_syntax(file_id, offset)
    }

    pub fn parse_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.parse_diagnostics(file_id)
    }

    pub fn file_compilation_diagnostics(&self, file_id: FileId) -> Arc<[CompilationDiagnostic]> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.file_compilation_diagnostics(file_id)
    }

    pub fn macro_expansion(&self, macro_file: MacroFileId) -> Arc<ExpandResult<ExpansionInfo>> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.macro_expansion(macro_file)
    }

    pub fn parse(&self, file_id: HirFileId) -> SyntaxTree {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.parse(file_id)
    }

    pub fn hir_file_with_source_map(&self, file_id: HirFileId) -> Arc<Lowered<HirFile>> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.hir_file_with_source_map(file_id)
    }

    pub fn hir_file(&self, file_id: HirFileId) -> Arc<HirFile> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.hir_file(file_id)
    }

    pub fn module_with_source_map(&self, module_id: ModuleId) -> Arc<Lowered<Module>> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.module_with_source_map(module_id)
    }

    pub fn module(&self, module_id: ModuleId) -> Arc<Module> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.module(module_id)
    }

    pub fn block_with_source_map(&self, block_id: BlockId) -> Arc<Lowered<Block>> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.block_with_source_map(block_id)
    }

    pub fn block(&self, block_id: BlockId) -> Arc<Block> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.block(block_id)
    }

    pub fn subroutine_with_source_map(
        &self,
        subroutine_id: SubroutineScope,
    ) -> Arc<Lowered<Subroutine>> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.subroutine_with_source_map(subroutine_id)
    }

    pub fn subroutine(&self, subroutine_id: SubroutineScope) -> Arc<Subroutine> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.subroutine(subroutine_id)
    }

    pub fn generate_block_with_source_map(
        &self,
        generate_block_id: GenerateBlockId,
    ) -> Arc<Lowered<GenerateBlock>> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.generate_block_with_source_map(generate_block_id)
    }

    pub fn generate_block(&self, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.generate_block(generate_block_id)
    }

    pub fn scope_for(&self, scope_id: ScopeId) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.scope_for(scope_id)
    }

    pub fn unit_scope(&self) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.unit_scope()
    }

    pub fn file_scope(&self, file_id: HirFileId) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.file_scope(file_id)
    }

    pub fn module_scope(&self, module_id: ModuleId) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.module_scope(module_id)
    }

    pub fn clocking_block_scope(
        &self,
        clocking_block_id: InModule<ClockingBlockId>,
    ) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.clocking_block_scope(clocking_block_id)
    }

    pub fn checker_scope(&self, checker_id: InFileOrModule<CheckerId>) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.checker_scope(checker_id)
    }

    pub fn covergroup_scope(&self, covergroup_id: InFileOrModule<CovergroupId>) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.covergroup_scope(covergroup_id)
    }

    pub fn generate_block_scope(&self, generate_block_id: GenerateBlockId) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.generate_block_scope(generate_block_id)
    }

    pub fn block_scope(&self, block_id: BlockId) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.block_scope(block_id)
    }

    pub fn subroutine_scope(&self, subroutine_id: SubroutineScope) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.subroutine_scope(subroutine_id)
    }

    pub fn package_export_signature(&self, package_id: PackageId) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.package_export_signature(package_id)
    }

    pub fn package_export_scope(&self, package_id: PackageId) -> Arc<NameScope> {
        let db: &dyn WorkspaceSymbolIndexDb = self;
        db.package_export_scope(package_id)
    }
}
