use syntax::SyntaxTree;
use triomphe::Arc;

use crate::{
    base_db::{salsa, source_db::SourceRootDb},
    container::{InContainer, InModule, InSubroutine},
    def_id::{ModuleDef, ModuleDefId},
    file::HirFileId,
    hir_def::{
        block::{self, Block, BlockId, BlockLoc, BlockSourceMap},
        expr::{
            ExprId,
            data_ty::{BuiltinDataTy, BuiltinDataTyId},
            declarator::DeclId,
        },
        file::{self, FileSourceMap, HirFile},
        macro_file::{self, ExpansionInfo, MacroCallId, MacroCallLoc, MacroFileId, MacroFileLoc},
        module::{
            self, Module, ModuleId, ModuleSourceMap, PackageId,
            clocking::ClockingBlockId,
            generate::{
                self, GenerateBlock, GenerateBlockId, GenerateBlockLoc, GenerateBlockSourceMap,
            },
        },
        subroutine::{self, LocalSubroutineId, Subroutine, SubroutinePortId, SubroutineSourceMap},
        typedef::TypedefId,
    },
    impl_intern_key, impl_intern_lookup,
    semantics::pathres::PathResolution,
    symbol::{DefId, DefLoc, NameScope},
    type_infer::TyResult,
};

pub(crate) macro impl_intern($id:ident, $loc:ident, $intern:ident, $lookup:ident) {
    impl_intern_key!($id);
    impl_intern_lookup!(InternDb, $id, $loc, $intern, $lookup);
}

#[salsa::query_group(InternDbStorage)]
pub trait InternDb: SourceRootDb {
    #[salsa::interned]
    fn intern_ty(&self, ty: BuiltinDataTy) -> BuiltinDataTyId;

    #[salsa::interned]
    fn intern_block(&self, block: BlockLoc) -> BlockId;

    #[salsa::interned]
    fn intern_generate_block(&self, generate_block: GenerateBlockLoc) -> GenerateBlockId;

    #[salsa::interned]
    fn intern_macro_call(&self, macro_call: MacroCallLoc) -> MacroCallId;

    #[salsa::interned]
    fn intern_macro_file(&self, macro_file: MacroFileLoc) -> MacroFileId;

    #[salsa::interned]
    fn intern_module_def(&self, module_def: ModuleDef) -> ModuleDefId;

    #[salsa::interned]
    fn intern_def(&self, def: DefLoc) -> DefId;
}

impl_intern!(BuiltinDataTyId, BuiltinDataTy, intern_ty, lookup_intern_ty);
impl_intern!(BlockId, BlockLoc, intern_block, lookup_intern_block);
impl_intern!(
    GenerateBlockId,
    GenerateBlockLoc,
    intern_generate_block,
    lookup_intern_generate_block
);
impl_intern!(MacroCallId, MacroCallLoc, intern_macro_call, lookup_intern_macro_call);
impl_intern!(MacroFileId, MacroFileLoc, intern_macro_file, lookup_intern_macro_file);
impl_intern!(ModuleDefId, ModuleDef, intern_module_def, lookup_intern_module_def);
impl_intern!(DefId, DefLoc, intern_def, lookup_intern_def);

#[salsa::query_group(HirDbStorage)]
pub trait HirDb: InternDb {
    #[salsa::transparent]
    fn parse(&self, file_id: HirFileId) -> SyntaxTree;

    #[salsa::invoke(macro_file::macro_expansion_query)]
    fn macro_expansion(&self, macro_file: MacroFileId) -> Arc<ExpansionInfo>;

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
        subroutine: InContainer<LocalSubroutineId>,
    ) -> (Arc<Subroutine>, Arc<SubroutineSourceMap>);

    fn subroutine(&self, subroutine_id: InContainer<LocalSubroutineId>) -> Arc<Subroutine>;

    #[salsa::invoke(generate::generate_block_with_source_map_query)]
    fn generate_block_with_source_map(
        &self,
        generate_block_id: GenerateBlockId,
    ) -> (Arc<GenerateBlock>, Arc<GenerateBlockSourceMap>);

    fn generate_block(&self, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock>;

    #[salsa::invoke(NameScope::unit_scope_query)]
    fn unit_scope(&self) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::file_scope_query)]
    fn file_scope(&self, file_id: HirFileId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::module_scope_query)]
    fn module_scope(&self, module_id: ModuleId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::clocking_block_scope_query)]
    fn clocking_block_scope(&self, clocking_block_id: InModule<ClockingBlockId>) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::generate_block_scope_query)]
    fn generate_block_scope(&self, generate_block_id: GenerateBlockId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::block_scope_query)]
    fn block_scope(&self, block_id: BlockId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::subroutine_scope_query)]
    fn subroutine_scope(&self, subroutine_id: InContainer<LocalSubroutineId>) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::package_export_signature_query)]
    fn package_export_signature(&self, package_id: PackageId) -> Arc<NameScope>;

    #[salsa::invoke(NameScope::package_export_scope_query)]
    fn package_export_scope(&self, package_id: PackageId) -> Arc<NameScope>;

    #[salsa::invoke(crate::type_infer::type_of_decl_query)]
    fn type_of_decl(&self, decl: InContainer<DeclId>) -> Arc<TyResult>;

    #[salsa::invoke(crate::type_infer::type_of_typedef_query)]
    fn type_of_typedef(&self, typedef: InContainer<TypedefId>) -> Arc<TyResult>;

    #[salsa::invoke(crate::type_infer::type_of_expr_query)]
    fn type_of_expr(&self, expr: InContainer<ExprId>) -> Arc<TyResult>;

    #[salsa::invoke(crate::type_infer::type_of_path_resolution_query)]
    fn type_of_path_resolution(&self, res: PathResolution) -> Arc<TyResult>;

    #[salsa::invoke(crate::type_infer::type_of_subroutine_port_query)]
    fn type_of_subroutine_port(&self, port: InSubroutine<SubroutinePortId>) -> Arc<TyResult>;
}

fn parse(db: &dyn HirDb, file_id: HirFileId) -> SyntaxTree {
    match file_id {
        HirFileId::File(file_id) => db.parse_src_for_compilation(file_id),
        HirFileId::Macro(macro_file) => db.macro_expansion(macro_file).parse.clone(),
    }
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

fn subroutine(db: &dyn HirDb, subroutine_id: InContainer<LocalSubroutineId>) -> Arc<Subroutine> {
    db.subroutine_with_source_map(subroutine_id).0
}

fn generate_block(db: &dyn HirDb, generate_block_id: GenerateBlockId) -> Arc<GenerateBlock> {
    db.generate_block_with_source_map(generate_block_id).0
}
