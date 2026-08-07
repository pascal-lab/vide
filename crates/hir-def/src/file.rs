use config::{ConfigDecl, ConfigDeclId, ConfigDeclSrc};
use la_arena::{Arena, Idx};
use library::{
    LibraryDecl, LibraryDeclId, LibraryDeclSrc, LibraryInclude, LibraryIncludeId, LibraryIncludeSrc,
};
pub use preproc_expand::file::HirFileId;
use smallvec::SmallVec;
use syntax::{
    ast::{self, AstNode},
    ptr::SyntaxNodePtr,
};
use triomphe::Arc;
use udp::{UdpDecl, UdpDeclId, UdpDeclSrc};
use utils::{define_enum_deriving_from, get::Get};

use super::{
    PackageImport,
    aggregate::{StructId, lower_struct_def},
    alloc_with_source,
    checker::{CheckerDef, CheckerId, CheckerSrc},
    covergroup::{
        CovergroupDef, CovergroupId, CovergroupSrc, CoverpointDef, CoverpointId, CoverpointSrc,
        CrossDef, CrossId, CrossSrc, lower_covergroup_decl, lower_coverpoint, lower_cross,
    },
    declaration::DeclarationId,
    lower::{FileStore, LoweringCtx},
    lower_package_imports,
    module::{LocalModuleId, ModuleInfo, ModuleKind, ModuleSrc},
    proc::{Proc, ProcId, ProcSrc},
    subroutine::{LocalSubroutineId, Subroutine, SubroutineSrc, lower_subroutine},
    typedef::{Typedef, TypedefId, lower_typedef_data_ty},
};
use crate::{
    ast_id_map::SyntaxFileId,
    body::{Body, BodySourceMap, OwnerLowering},
    db::HirDefDb,
    lower_ident_opt,
    region_tree::RegionTree,
    source_map::{DiagnosticSource, Lowered, LoweredData, LoweringDiagnostic, SourceMap},
};

pub mod config;
pub mod library;
pub mod udp;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct HirFile {
    pub items: SmallVec<[FileItem; 3]>,
    pub modules: Arena<ModuleInfo>,
    pub procs: Arena<Proc>,
    pub config_decls: Arena<ConfigDecl>,
    pub udp_decls: Arena<UdpDecl>,
    pub library_decls: Arena<LibraryDecl>,
    pub library_includes: Arena<LibraryInclude>,
    pub checkers: Arena<CheckerDef>,
    pub covergroups: Arena<CovergroupDef>,
    pub coverpoints: Arena<CoverpointDef>,
    pub crosses: Arena<CrossDef>,
    pub subroutines: Arena<Subroutine>,
    pub package_imports: Arena<PackageImport>,
}
impl HirFile {
    pub fn shrink_to_fit(&mut self) {
        self.modules.shrink_to_fit();
        self.procs.shrink_to_fit();
        self.config_decls.shrink_to_fit();
        self.udp_decls.shrink_to_fit();
        self.library_decls.shrink_to_fit();
        self.library_includes.shrink_to_fit();
        self.checkers.shrink_to_fit();
        self.covergroups.shrink_to_fit();
        self.coverpoints.shrink_to_fit();
        self.crosses.shrink_to_fit();
        self.subroutines.shrink_to_fit();
        self.package_imports.shrink_to_fit();
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct FileSourceMap {
    pub region_tree: RegionTree,
    pub module_srcs: SourceMap<ModuleInfo>,
    pub proc_srcs: SourceMap<Proc>,
    pub config_decl_srcs: SourceMap<ConfigDecl>,
    pub udp_decl_srcs: SourceMap<UdpDecl>,
    pub library_decl_srcs: SourceMap<LibraryDecl>,
    pub library_include_srcs: SourceMap<LibraryInclude>,
    pub checker_srcs: SourceMap<CheckerDef>,
    pub covergroup_srcs: SourceMap<CovergroupDef>,
    pub coverpoint_srcs: SourceMap<CoverpointDef>,
    pub cross_srcs: SourceMap<CrossDef>,
    pub subroutine_srcs: SourceMap<Subroutine>,
    pub diagnostics: Vec<LoweringDiagnostic>,
}
impl LoweredData for HirFile {
    type SourceMap = FileSourceMap;
}

impl DiagnosticSource for FileSourceMap {
    fn diagnostics(&self) -> &[LoweringDiagnostic] {
        &self.diagnostics
    }
}

impl FileSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.module_srcs.shrink_to_fit();
        self.proc_srcs.shrink_to_fit();
        self.config_decl_srcs.shrink_to_fit();
        self.udp_decl_srcs.shrink_to_fit();
        self.library_decl_srcs.shrink_to_fit();
        self.library_include_srcs.shrink_to_fit();
        self.checker_srcs.shrink_to_fit();
        self.covergroup_srcs.shrink_to_fit();
        self.coverpoint_srcs.shrink_to_fit();
        self.cross_srcs.shrink_to_fit();
        self.subroutine_srcs.shrink_to_fit();
        self.diagnostics.shrink_to_fit();
    }
}

crate::impl_arena_getters!(
    HirFile;
    LocalModuleId => modules => ModuleInfo,
    ProcId => procs => Proc,
    ConfigDeclId => config_decls => ConfigDecl,
    UdpDeclId => udp_decls => UdpDecl,
    LibraryDeclId => library_decls => LibraryDecl,
    LibraryIncludeId => library_includes => LibraryInclude,
    CheckerId => checkers => CheckerDef,
    CovergroupId => covergroups => CovergroupDef,
    CoverpointId => coverpoints => CoverpointDef,
    CrossId => crosses => CrossDef,
    LocalSubroutineId => subroutines => Subroutine,
    Idx<PackageImport> => package_imports => PackageImport,
);

crate::impl_source_map_getters!(
    FileSourceMap;
    LocalModuleId => module_srcs,
    ProcId => proc_srcs,
    ConfigDeclId => config_decl_srcs,
    UdpDeclId => udp_decl_srcs,
    LibraryDeclId => library_decl_srcs,
    LibraryIncludeId => library_include_srcs,
    CheckerId => checker_srcs,
    CovergroupId => covergroup_srcs,
    CoverpointId => coverpoint_srcs,
    CrossId => cross_srcs,
    LocalSubroutineId => subroutine_srcs,
);

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum FileItem {
        LocalModuleId(LocalModuleId),
        ProcId(ProcId),
        DeclarationId(DeclarationId),
        TypedefId(TypedefId),
        StructId(StructId),
        ConfigDeclId(ConfigDeclId),
        UdpDeclId(UdpDeclId),
        LibraryDeclId(LibraryDeclId),
        LibraryIncludeId(LibraryIncludeId),
        CheckerId(CheckerId),
        CovergroupId(CovergroupId),
        SubroutineId(LocalSubroutineId),
    }
}

impl FileSourceMap {
    pub fn item_to_source(
        &self,
        body: &BodySourceMap,
        item: &FileItem,
    ) -> Option<crate::ast_id_map::SourceAstId> {
        match item {
            FileItem::LocalModuleId(idx) => self.get(*idx),
            FileItem::ProcId(idx) => self.get(*idx),
            FileItem::DeclarationId(idx) => body.declaration_srcs.hir_to_src(*idx),
            FileItem::TypedefId(idx) => body.typedef_srcs.hir_to_src(*idx),
            FileItem::StructId(idx) => body.struct_srcs.hir_to_src(*idx),
            FileItem::ConfigDeclId(idx) => self.get(*idx),
            FileItem::UdpDeclId(idx) => self.get(*idx),
            FileItem::LibraryDeclId(idx) => self.get(*idx),
            FileItem::LibraryIncludeId(idx) => self.get(*idx),
            FileItem::CheckerId(idx) => self.get(*idx),
            FileItem::CovergroupId(idx) => self.get(*idx),
            FileItem::SubroutineId(idx) => self.get(*idx),
        }
    }
}

pub(crate) type LowerFileCtx<'a> = LoweringCtx<FileStore<'a>>;

impl LowerFileCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = self.current_arena_owner();
        let struct_def = lower_struct_def(struct_ty, container_id, |ty| self.lower_data_ty(ty));

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.body.structs,
            &mut self.store.body_sources.struct_srcs,
            struct_def,
            struct_ty,
        )
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());
        let typedef_id = alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.body.typedefs,
            &mut self.store.body_sources.typedef_srcs,
            Typedef { name, ty: None },
            typedef,
        );
        self.record_body_typedef(typedef_id);

        let data_ty = typedef.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            self.current_arena_owner(),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.lower_data_ty(ty),
        );

        self.store.body.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_subroutine_decl(
        &mut self,
        func: ast::FunctionDeclaration,
    ) -> Option<LocalSubroutineId> {
        // Only the skeleton is lowered here; the body is lowered on first
        // access by subroutine_body_with_source_map.
        let subroutine = lower_subroutine(&func, |ty| self.lower_data_ty(ty))?;

        let local_subroutine_id = alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.subroutines,
            &mut self.store.sources.subroutine_srcs,
            subroutine,
            func,
        );

        Some(local_subroutine_id)
    }

    fn lower_config_decl(&mut self, config_decl: ast::ConfigDeclaration) -> ConfigDeclId {
        let name = lower_ident_opt(config_decl.name());

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.config_decls,
            &mut self.store.sources.config_decl_srcs,
            ConfigDecl { name },
            config_decl,
        )
    }

    fn lower_udp_decl(&mut self, udp_decl: ast::UdpDeclaration) -> UdpDeclId {
        let name = lower_ident_opt(udp_decl.name());

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.udp_decls,
            &mut self.store.sources.udp_decl_srcs,
            UdpDecl { name },
            udp_decl,
        )
    }

    fn lower_library_decl(&mut self, library_decl: ast::LibraryDeclaration) -> LibraryDeclId {
        let name = lower_ident_opt(library_decl.name());

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.library_decls,
            &mut self.store.sources.library_decl_srcs,
            LibraryDecl { name },
            library_decl,
        )
    }

    fn lower_library_include(
        &mut self,
        library_include: ast::LibraryIncludeStatement,
    ) -> LibraryIncludeId {
        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.library_includes,
            &mut self.store.sources.library_include_srcs,
            LibraryInclude,
            library_include,
        )
    }

    fn lower_covergroup_decl(
        &mut self,
        covergroup_decl: ast::CovergroupDeclaration,
    ) -> CovergroupId {
        let mut covergroup = lower_covergroup_decl(covergroup_decl);

        for member in covergroup_decl.members().children() {
            match member {
                ast::Member::Coverpoint(coverpoint_ast) => {
                    let coverpoint = lower_coverpoint(coverpoint_ast);
                    let coverpoint_id = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.coverpoints,
                        &mut self.store.sources.coverpoint_srcs,
                        coverpoint,
                        coverpoint_ast,
                    );
                    covergroup.coverpoints.push(coverpoint_id);
                }
                ast::Member::CoverCross(cross_ast) => {
                    let cross = lower_cross(cross_ast);
                    let cross_id = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.crosses,
                        &mut self.store.sources.cross_srcs,
                        cross,
                        cross_ast,
                    );
                    covergroup.crosses.push(cross_id);
                }
                _ => {}
            }
        }

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.covergroups,
            &mut self.store.sources.covergroup_srcs,
            covergroup,
            covergroup_decl,
        )
    }

    pub(crate) fn lower_file(&mut self, root: ast::CompilationUnit) {
        for member in root.members().children() {
            use ast::Member::*;
            let idx = match member {
                ModuleDeclaration(decl) => {
                    let name = lower_ident_opt(decl.header().name());
                    let kind = ModuleKind::from_ast(decl);

                    alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.modules,
                        &mut self.store.sources.module_srcs,
                        ModuleInfo { name, kind },
                        decl,
                    )
                    .into()
                }
                ProceduralBlock(proc) => self.lower_proc(proc).into(),
                DataDeclaration(data_decl) => self.lower_data_decl(data_decl).into(),
                NetDeclaration(net_decl) => self.lower_net_decl(net_decl).into(),
                EmptyMember(_x) => continue,
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                FunctionDeclaration(fn_decl) => match self.lower_subroutine_decl(fn_decl) {
                    Some(id) => id.into(),
                    None => continue,
                },
                PackageImportDeclaration(import_decl) => {
                    for import in lower_package_imports(import_decl) {
                        self.store.data.package_imports.alloc(import);
                    }
                    continue;
                }
                UdpDeclaration(udp_decl) => self.lower_udp_decl(udp_decl).into(),
                ConfigDeclaration(config_decl) => self.lower_config_decl(config_decl).into(),
                CheckerDeclaration(checker_decl) => self.lower_checker_decl(checker_decl).into(),
                CovergroupDeclaration(covergroup_decl) => {
                    self.lower_covergroup_decl(covergroup_decl).into()
                }
                _ => continue,
            };
            self.store.data.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }

        self.region_tree.stage(root.end_of_file(), root.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }

    pub(crate) fn lower_library_map(&mut self, root: ast::LibraryMap) {
        for member in root.members().children() {
            use ast::Member::*;
            let idx = match member {
                LibraryDeclaration(library_decl) => self.lower_library_decl(library_decl).into(),
                LibraryIncludeStatement(library_include) => {
                    self.lower_library_include(library_include).into()
                }
                EmptyMember(_) => continue,
                _ => continue,
            };
            self.store.data.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }

        self.region_tree.stage(root.end_of_file(), root.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
fn file_lowering(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<OwnerLowering<HirFile>> {
    let file_id = file.hir_file(db);
    let mut hir_file = HirFile::default();
    let mut source_map = FileSourceMap::default();
    let mut body = Body::default();
    let mut body_source_map = BodySourceMap::default();

    let tree = db.parse(file_id);
    let owner = db.owner_table(file_id).file_owner().expect("file owner must exist");
    let mut lower_ctx = LoweringCtx::new(
        db,
        owner,
        FileStore {
            data: &mut hir_file,
            sources: &mut source_map,
            body: &mut body,
            body_sources: &mut body_source_map,
        },
    );
    match tree.root() {
        Some(root) if ast::CompilationUnit::can_cast(root.kind()) => {
            if let Some(root) = ast::CompilationUnit::cast(root) {
                lower_ctx.lower_file(root);
            }
        }
        Some(root) if ast::LibraryMap::can_cast(root.kind()) => {
            if let Some(root) = ast::LibraryMap::cast(root) {
                lower_ctx.lower_library_map(root);
            }
        }
        _ => {}
    }

    let diagnostics = lower_ctx.emit_diagnostics();
    drop(lower_ctx);
    source_map.diagnostics = diagnostics.clone();
    body_source_map.diagnostics = diagnostics;

    hir_file.shrink_to_fit();
    source_map.shrink_to_fit();
    body.shrink_to_fit();
    body_source_map.shrink_to_fit();
    Arc::new(OwnerLowering::new(file_id, hir_file, source_map, body, body_source_map))
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn hir_file_with_source_map(
    db: &dyn HirDefDb,
    file: SyntaxFileId,
) -> Arc<Lowered<HirFile>> {
    file_lowering(db, file).structure.clone()
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn file_body_with_source_map(
    db: &dyn HirDefDb,
    file: SyntaxFileId,
) -> Arc<Lowered<Body>> {
    file_lowering(db, file).body.clone()
}

pub(crate) fn set_hir_file_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    file_lowering::set_lru_capacity(db, capacity);
    hir_file_with_source_map::set_lru_capacity(db, capacity);
    file_body_with_source_map::set_lru_capacity(db, capacity);
}
