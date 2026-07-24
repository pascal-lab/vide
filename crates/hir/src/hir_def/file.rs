use config::{ConfigDecl, ConfigDeclId, ConfigDeclSrc};
use la_arena::{Arena, Idx};
use library::{
    LibraryDecl, LibraryDeclId, LibraryDeclSrc, LibraryInclude, LibraryIncludeId, LibraryIncludeSrc,
};
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
    aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
    alloc_with_source,
    block::{BlockInfo, BlockSrc, LocalBlockId},
    checker::{CheckerDef, CheckerId, CheckerSrc},
    covergroup::{
        CovergroupDef, CovergroupId, CovergroupSrc, CoverpointDef, CoverpointId, CoverpointSrc,
        CrossDef, CrossId, CrossSrc, lower_covergroup_decl, lower_coverpoint, lower_cross,
    },
    declaration::{Declaration, DeclarationId, DeclarationSrc},
    expr::{
        Expr, ExprId, ExprSrc,
        declarator::{DeclId, Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprId, EventExprSrc},
    },
    lower::{FileStore, LoweringCtx, SubroutineStore},
    lower_package_imports,
    module::{LocalModuleId, ModuleInfo, ModuleKind, ModuleSrc},
    proc::{Proc, ProcId, ProcSrc},
    stmt::{Stmt, StmtId, StmtSrc},
    subroutine::{
        LocalSubroutineId, Subroutine, SubroutineSrc, lower_subroutine, lower_subroutine_body,
    },
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::{InContainer, ScopeId},
    db::HirDb,
    file::HirFileId,
    hir_def::lower_ident_opt,
    region_tree::RegionTree,
    source_map::SourceMap,
};

pub mod config;
pub mod library;
pub mod udp;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct HirFile {
    pub modules: Arena<ModuleInfo>,
    pub procs: Arena<Proc>,
    pub typedefs: Arena<Typedef>,
    pub structs: Arena<StructDef>,
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
    pub declarations: Arena<Declaration>,
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
    pub stmts: Arena<Stmt>,
}

impl HirFile {
    pub fn shrink_to_fit(&mut self) {
        self.modules.shrink_to_fit();
        self.procs.shrink_to_fit();
        self.typedefs.shrink_to_fit();
        self.structs.shrink_to_fit();
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
        self.declarations.shrink_to_fit();
        self.exprs.shrink_to_fit();
        self.event_exprs.shrink_to_fit();
        self.decls.shrink_to_fit();
        self.stmts.shrink_to_fit();
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct FileSourceMap {
    pub items: SmallVec<[FileItem; 3]>,
    pub region_tree: RegionTree,
    pub module_srcs: SourceMap<ModuleSrc, ModuleInfo>,
    pub proc_srcs: SourceMap<ProcSrc, Proc>,
    pub declaration_srcs: SourceMap<DeclarationSrc, Declaration>,
    pub typedef_srcs: SourceMap<TypedefSrc, Typedef>,
    pub struct_srcs: SourceMap<StructSrc, StructDef>,
    pub config_decl_srcs: SourceMap<ConfigDeclSrc, ConfigDecl>,
    pub udp_decl_srcs: SourceMap<UdpDeclSrc, UdpDecl>,
    pub library_decl_srcs: SourceMap<LibraryDeclSrc, LibraryDecl>,
    pub library_include_srcs: SourceMap<LibraryIncludeSrc, LibraryInclude>,
    pub checker_srcs: SourceMap<CheckerSrc, CheckerDef>,
    pub covergroup_srcs: SourceMap<CovergroupSrc, CovergroupDef>,
    pub coverpoint_srcs: SourceMap<CoverpointSrc, CoverpointDef>,
    pub cross_srcs: SourceMap<CrossSrc, CrossDef>,
    pub subroutine_srcs: SourceMap<SubroutineSrc, Subroutine>,
    pub expr_srcs: SourceMap<ExprSrc, Expr>,
    pub event_expr_srcs: SourceMap<EventExprSrc, EventExpr>,
    pub decl_srcs: SourceMap<DeclaratorSrc, Declarator>,
    pub stmt_srcs: SourceMap<StmtSrc, Stmt>,
}

impl FileSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.module_srcs.shrink_to_fit();
        self.proc_srcs.shrink_to_fit();
        self.declaration_srcs.shrink_to_fit();
        self.typedef_srcs.shrink_to_fit();
        self.struct_srcs.shrink_to_fit();
        self.config_decl_srcs.shrink_to_fit();
        self.udp_decl_srcs.shrink_to_fit();
        self.library_decl_srcs.shrink_to_fit();
        self.library_include_srcs.shrink_to_fit();
        self.checker_srcs.shrink_to_fit();
        self.covergroup_srcs.shrink_to_fit();
        self.coverpoint_srcs.shrink_to_fit();
        self.cross_srcs.shrink_to_fit();
        self.subroutine_srcs.shrink_to_fit();
        self.expr_srcs.shrink_to_fit();
        self.event_expr_srcs.shrink_to_fit();
        self.decl_srcs.shrink_to_fit();
        self.stmt_srcs.shrink_to_fit();
    }
}

crate::hir_def::impl_arena_getters!(
    HirFile;
    LocalModuleId => modules => ModuleInfo,
    ProcId => procs => Proc,
    TypedefId => typedefs => Typedef,
    StructId => structs => StructDef,
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
    DeclarationId => declarations => Declaration,
    ExprId => exprs => Expr,
    EventExprId => event_exprs => EventExpr,
    DeclId => decls => Declarator,
    StmtId => stmts => Stmt,
    LocalBlockId => stmts => BlockInfo,
);

crate::hir_def::impl_source_map_getters!(
    FileSourceMap;
    ModuleSrc => LocalModuleId => module_srcs,
    ProcSrc => ProcId => proc_srcs,
    DeclarationSrc => DeclarationId => declaration_srcs,
    TypedefSrc => TypedefId => typedef_srcs,
    StructSrc => StructId => struct_srcs,
    ConfigDeclSrc => ConfigDeclId => config_decl_srcs,
    UdpDeclSrc => UdpDeclId => udp_decl_srcs,
    LibraryDeclSrc => LibraryDeclId => library_decl_srcs,
    LibraryIncludeSrc => LibraryIncludeId => library_include_srcs,
    CheckerSrc => CheckerId => checker_srcs,
    CovergroupSrc => CovergroupId => covergroup_srcs,
    CoverpointSrc => CoverpointId => coverpoint_srcs,
    CrossSrc => CrossId => cross_srcs,
    SubroutineSrc => LocalSubroutineId => subroutine_srcs,
    ExprSrc => ExprId => expr_srcs,
    EventExprSrc => EventExprId => event_expr_srcs,
    DeclaratorSrc => DeclId => decl_srcs,
    StmtSrc => StmtId => stmt_srcs,
    BlockSrc => LocalBlockId => stmt_srcs,
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
    pub fn item_to_ptr(&self, item: &FileItem) -> Option<SyntaxNodePtr> {
        Some(match item {
            FileItem::LocalModuleId(idx) => self.get(*idx)?.node,
            FileItem::ProcId(idx) => self.get(*idx)?.0,
            FileItem::DeclarationId(idx) => self.get(*idx)?.ptr(),
            FileItem::TypedefId(idx) => self.get(*idx)?.ptr(),
            FileItem::StructId(idx) => self.get(*idx)?.node,
            FileItem::ConfigDeclId(idx) => self.get(*idx)?.node,
            FileItem::UdpDeclId(idx) => self.get(*idx)?.node,
            FileItem::LibraryDeclId(idx) => self.get(*idx)?.node,
            FileItem::LibraryIncludeId(idx) => self.get(*idx)?.0,
            FileItem::CheckerId(idx) => self.get(*idx)?.node,
            FileItem::CovergroupId(idx) => self.get(*idx)?.node,
            FileItem::SubroutineId(idx) => self.get(*idx)?.node,
        })
    }
}

pub(crate) type LowerFileCtx<'a> = LoweringCtx<'a, FileStore<'a>>;

impl LowerFileCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = ScopeId::File(self.file_id);
        let struct_def = lower_struct_def(struct_ty, container_id, |ty| self.lower_data_ty(ty));

        alloc_with_source(
            self.file_id,
            &mut self.store.data.structs,
            &mut self.store.sources.struct_srcs,
            struct_def,
            struct_ty,
        )
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());
        let typedef_id = alloc_with_source(
            self.file_id,
            &mut self.store.data.typedefs,
            &mut self.store.sources.typedef_srcs,
            Typedef { name, ty: None },
            typedef,
        );

        let data_ty = typedef.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            ScopeId::File(self.file_id),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.lower_data_ty(ty),
        );

        self.store.data.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_subroutine_decl(
        &mut self,
        func: ast::FunctionDeclaration,
    ) -> Option<LocalSubroutineId> {
        let subroutine = lower_subroutine(&func, |ty| self.lower_data_ty(ty))?;

        let local_subroutine_id = alloc_with_source(
            self.file_id,
            &mut self.store.data.subroutines,
            &mut self.store.sources.subroutine_srcs,
            subroutine,
            func,
        );

        let subroutine_id = InContainer::new(self.file_id.into(), local_subroutine_id);

        if func.end().is_some() {
            let subroutine = &mut self.store.data.subroutines[local_subroutine_id];
            let mut subroutine_source_map = std::mem::take(&mut subroutine.source_map);
            let mut ctx = LoweringCtx::new(
                self.db,
                self.file_id,
                subroutine_id.into(),
                SubroutineStore { data: subroutine, sources: &mut subroutine_source_map },
            );
            lower_subroutine_body(&mut ctx, func);
            ctx.emit_diagnostics();
            drop(ctx);
            subroutine.source_map = subroutine_source_map;
            subroutine.source_map.shrink_to_fit();
        }

        self.store.data.subroutines[local_subroutine_id].shrink_to_fit();

        Some(local_subroutine_id)
    }

    fn lower_config_decl(&mut self, config_decl: ast::ConfigDeclaration) -> ConfigDeclId {
        let name = lower_ident_opt(config_decl.name());

        alloc_with_source(
            self.file_id,
            &mut self.store.data.config_decls,
            &mut self.store.sources.config_decl_srcs,
            ConfigDecl { name },
            config_decl,
        )
    }

    fn lower_udp_decl(&mut self, udp_decl: ast::UdpDeclaration) -> UdpDeclId {
        let name = lower_ident_opt(udp_decl.name());

        alloc_with_source(
            self.file_id,
            &mut self.store.data.udp_decls,
            &mut self.store.sources.udp_decl_srcs,
            UdpDecl { name },
            udp_decl,
        )
    }

    fn lower_library_decl(&mut self, library_decl: ast::LibraryDeclaration) -> LibraryDeclId {
        let name = lower_ident_opt(library_decl.name());

        alloc_with_source(
            self.file_id,
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
            self.file_id,
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
                        self.file_id,
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
                        self.file_id,
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
            self.file_id,
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
                        self.file_id,
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
            self.store.sources.items.push(idx);
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
            self.store.sources.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }

        self.region_tree.stage(root.end_of_file(), root.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }
}

pub(crate) fn hir_file_with_source_map_query(
    db: &dyn HirDb,
    file_id: HirFileId,
) -> (Arc<HirFile>, Arc<FileSourceMap>) {
    let mut hir_file = HirFile::default();
    let mut source_map = FileSourceMap::default();

    let tree = db.parse(file_id);
    let mut lower_ctx = LoweringCtx::new(
        db,
        file_id,
        file_id.into(),
        FileStore { data: &mut hir_file, sources: &mut source_map },
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

    lower_ctx.emit_diagnostics();
    drop(lower_ctx);

    hir_file.shrink_to_fit();
    source_map.shrink_to_fit();

    (Arc::new(hir_file), Arc::new(source_map))
}
