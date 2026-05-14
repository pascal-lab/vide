use config::{ConfigDecl, ConfigDeclId, ConfigDeclSrc};
use la_arena::Arena;
use library::{
    LibraryDecl, LibraryDeclId, LibraryDeclSrc, LibraryInclude, LibraryIncludeId, LibraryIncludeSrc,
};
use proc_macro_utils::define_container;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    ast::{self, AstNode},
    ptr::SyntaxNodePtr,
};
use triomphe::Arc;
use udp::{UdpDecl, UdpDeclId, UdpDeclSrc};
use utils::{define_enum_deriving_from, get::Get};

use super::{
    aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
    alloc_idx_and_src,
    block::{BlockInfo, BlockSrc, LocalBlockId},
    declaration::{
        Declaration, DeclarationId, DeclarationSrc, LowerDeclaration, impl_lower_declaration,
    },
    expr::{
        Expr, ExprSrc, LowerExpr,
        declarator::{Declarator, DeclaratorSrc, impl_lower_decl},
        impl_lower_expr,
        timing_control::{EventExpr, EventExprSrc, impl_lower_event_expr},
    },
    module::{LocalModuleId, ModuleInfo, ModuleSrc},
    opaque::{OpaqueItem, OpaqueItemId, OpaqueItemSrc, OpaqueKind, lower_opaque_member},
    proc::{LowerProc, LowerProcCtx, Proc, ProcId, ProcSrc},
    stmt::{Stmt, StmtId, StmtSrc, impl_lower_stmt},
    subroutine::{
        LocalSubroutineId, LowerSubroutineBodyCtx, Subroutine, SubroutineLoc, SubroutineSourceMap,
        SubroutineSrc, lower_subroutine, lower_subroutine_body,
    },
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::{ContainerId, InFile},
    db::{HirDb, InternDb},
    file::HirFileId,
    hir_def::lower_ident_opt,
    region_tree::{RegionTree, RegionTreeBuilder},
    source_map::SourceMap,
};

pub mod config;
pub mod library;
pub mod udp;

define_container! {
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct HirFile {
        modules: [ModuleInfo],
        procs: [Proc],

        typedefs: [Typedef],
        structs: [StructDef],
        config_decls: [ConfigDecl],
        udp_decls: [UdpDecl],
        library_decls: [LibraryDecl],
        library_includes: [LibraryInclude],
        opaque_items: [OpaqueItem],
        subroutines: [Subroutine],
        subroutine_source_maps: FxHashMap<LocalSubroutineId, SubroutineSourceMap>,

        declarations: [Declaration],
        exprs: [Expr],
        event_exprs: [EventExpr],
        decls: [Declarator],
        stmts: [Stmt] => {
            [StmtId | Stmt],
            [LocalBlockId | BlockInfo],
        },
    }
}

define_container! {
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct FileSourceMap {
        items: SmallVec<[FileItem; 3]>,
        region_tree: RegionTree,

        module_srcs: [ModuleInfo | ModuleSrc],
        proc_srcs: [Proc | ProcSrc],

        declaration_srcs: [Declaration | DeclarationSrc],
        typedef_srcs: [Typedef | TypedefSrc],
        struct_srcs: [StructDef | StructSrc],
        config_decl_srcs: [ConfigDecl | ConfigDeclSrc],
        udp_decl_srcs: [UdpDecl | UdpDeclSrc],
        library_decl_srcs: [LibraryDecl | LibraryDeclSrc],
        library_include_srcs: [LibraryInclude | LibraryIncludeSrc],
        opaque_srcs: [OpaqueItem | OpaqueItemSrc],
        subroutine_srcs: [Subroutine | SubroutineSrc],
        expr_srcs: [Expr | ExprSrc],
        event_expr_srcs: [EventExpr | EventExprSrc],
        decl_srcs: [Declarator | DeclaratorSrc],
        stmt_srcs: [Stmt | StmtSrc] => {
            [StmtId | StmtSrc],
            [LocalBlockId | BlockSrc],
        }
    }
}

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
        OpaqueItemId(OpaqueItemId),
        SubroutineId(LocalSubroutineId),
    }
}

impl FileSourceMap {
    pub fn item_to_ptr(&self, item: &FileItem) -> SyntaxNodePtr {
        match item {
            FileItem::LocalModuleId(idx) => self.get(*idx).node,
            FileItem::ProcId(idx) => self.get(*idx).0,
            FileItem::DeclarationId(idx) => self.get(*idx).ptr(),
            FileItem::TypedefId(idx) => self.get(*idx).ptr(),
            FileItem::StructId(idx) => self.get(*idx).node,
            FileItem::ConfigDeclId(idx) => self.get(*idx).node,
            FileItem::UdpDeclId(idx) => self.get(*idx).node,
            FileItem::LibraryDeclId(idx) => self.get(*idx).node,
            FileItem::LibraryIncludeId(idx) => self.get(*idx).0,
            FileItem::OpaqueItemId(idx) => self.get(*idx).node,
            FileItem::SubroutineId(idx) => self.get(*idx).node,
        }
    }
}

pub(crate) struct LowerFileCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,

    pub(crate) file: &'a mut HirFile,
    pub(crate) file_source_map: &'a mut FileSourceMap,

    pub(crate) region_tree: RegionTreeBuilder,
}

impl_lower_expr!(LowerFileCtx<'_>, file, file_source_map);
impl_lower_decl!(LowerFileCtx<'_>, file, file_source_map);
impl_lower_event_expr!(LowerFileCtx<'_>, file, file_source_map);
impl_lower_stmt!(LowerFileCtx<'_>, file_id, file, file_source_map);
impl_lower_declaration!(LowerFileCtx<'_>, file, file_source_map);

impl LowerProc for LowerFileCtx<'_> {
    fn proc_ctx(&mut self) -> LowerProcCtx<'_> {
        LowerProcCtx {
            db: self.db,
            file_id: self.file_id,
            cont_id: self.file_id.into(),
            procs: &mut self.file.procs,
            proc_srcs: &mut self.file_source_map.proc_srcs,

            stmts: &mut self.file.stmts,
            stmt_srcs: &mut self.file_source_map.stmt_srcs,

            exprs: &mut self.file.exprs,
            expr_srcs: &mut self.file_source_map.expr_srcs,

            event_exprs: &mut self.file.event_exprs,
            event_expr_srcs: &mut self.file_source_map.event_expr_srcs,

            decls: &mut self.file.decls,
            decl_srcs: &mut self.file_source_map.decl_srcs,
        }
    }
}

impl LowerFileCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = ContainerId::HirFileId(self.file_id);
        let struct_def =
            lower_struct_def(struct_ty, container_id, |ty| self.expr_ctx().lower_data_ty(ty));

        alloc_idx_and_src! {
            struct_def => self.file.structs,
            struct_ty => self.file_source_map.struct_srcs,
        }
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());
        let typedef_id = alloc_idx_and_src! {
            Typedef { name, ty: None } => self.file.typedefs,
            typedef => self.file_source_map.typedef_srcs,
        };

        let data_ty = typedef.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            ContainerId::HirFileId(self.file_id),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.expr_ctx().lower_data_ty(ty),
        );

        self.file.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_subroutine_decl(
        &mut self,
        func: ast::FunctionDeclaration,
    ) -> Option<LocalSubroutineId> {
        let src = SubroutineSrc::from(func);
        let subroutine_def_id = self.db.intern_subroutine(SubroutineLoc {
            cont_id: self.file_id.into(),
            src: InFile::new(self.file_id, src),
        });

        let subroutine = lower_subroutine(&func, |ty| self.expr_ctx().lower_data_ty(ty))?;

        let local_subroutine_id = alloc_idx_and_src! {
            subroutine => self.file.subroutines,
            func => self.file_source_map.subroutine_srcs,
        };

        if func.end().is_some() {
            let subroutine = &mut self.file.subroutines[local_subroutine_id];
            let mut subroutine_source_map = SubroutineSourceMap::default();
            let mut ctx = LowerSubroutineBodyCtx {
                db: self.db,
                file_id: self.file_id,
                subroutine_id: subroutine_def_id,
                subroutine,
                subroutine_source_map: &mut subroutine_source_map,
                region_tree: RegionTreeBuilder::new(),
            };
            lower_subroutine_body(&mut ctx, func);
            self.file.subroutine_source_maps.insert(local_subroutine_id, subroutine_source_map);
        }

        self.file.subroutines[local_subroutine_id].shrink_to_fit();

        Some(local_subroutine_id)
    }

    fn lower_config_decl(&mut self, config_decl: ast::ConfigDeclaration) -> ConfigDeclId {
        let name = lower_ident_opt(config_decl.name());

        alloc_idx_and_src! {
            ConfigDecl { name } => self.file.config_decls,
            config_decl => self.file_source_map.config_decl_srcs,
        }
    }

    fn lower_udp_decl(&mut self, udp_decl: ast::UdpDeclaration) -> UdpDeclId {
        let name = lower_ident_opt(udp_decl.name());

        alloc_idx_and_src! {
            UdpDecl { name } => self.file.udp_decls,
            udp_decl => self.file_source_map.udp_decl_srcs,
        }
    }

    fn lower_library_decl(&mut self, library_decl: ast::LibraryDeclaration) -> LibraryDeclId {
        let name = lower_ident_opt(library_decl.name());

        alloc_idx_and_src! {
            LibraryDecl { name } => self.file.library_decls,
            library_decl => self.file_source_map.library_decl_srcs,
        }
    }

    fn lower_library_include(
        &mut self,
        library_include: ast::LibraryIncludeStatement,
    ) -> LibraryIncludeId {
        alloc_idx_and_src! {
            LibraryInclude => self.file.library_includes,
            library_include => self.file_source_map.library_include_srcs,
        }
    }

    pub(crate) fn lower_file(&mut self, root: ast::CompilationUnit) {
        for member in root.members().children() {
            use ast::Member::*;
            let idx = match member {
                ModuleDeclaration(decl) => {
                    let name = lower_ident_opt(decl.header().name());

                    alloc_idx_and_src! {
                        ModuleInfo { name } => self.file.modules,
                        decl => self.file_source_map.module_srcs,
                    }
                    .into()
                }
                ProceduralBlock(proc) => self.proc_ctx().lower_proc(proc).into(),
                DataDeclaration(data_decl) => {
                    self.declaration_ctx().lower_data_decl(data_decl).into()
                }
                NetDeclaration(net_decl) => self.declaration_ctx().lower_net_decl(net_decl).into(),
                EmptyMember(_x) => continue,
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                FunctionDeclaration(fn_decl) => match self.lower_subroutine_decl(fn_decl) {
                    Some(id) => id.into(),
                    None => continue,
                },
                UdpDeclaration(udp_decl) => self.lower_udp_decl(udp_decl).into(),
                ConfigDeclaration(config_decl) => self.lower_config_decl(config_decl).into(),
                _ => {
                    let (opaque, src) = lower_opaque_member(member, OpaqueKind::FileItem);
                    let idx = self.file.opaque_items.alloc(opaque);
                    self.file_source_map.opaque_srcs.insert(src, idx);
                    idx.into()
                }
            };
            self.file_source_map.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }

        self.region_tree.stage(root.end_of_file());
        self.file_source_map.region_tree = self.region_tree.finish();
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
                _ => {
                    let (opaque, src) = lower_opaque_member(member, OpaqueKind::FileItem);
                    let idx = self.file.opaque_items.alloc(opaque);
                    self.file_source_map.opaque_srcs.insert(src, idx);
                    idx.into()
                }
            };
            self.file_source_map.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }

        self.region_tree.stage(root.end_of_file());
        self.file_source_map.region_tree = self.region_tree.finish();
    }
}

pub(crate) fn hir_file_with_source_map_query(
    db: &dyn HirDb,
    file_id: HirFileId,
) -> (Arc<HirFile>, Arc<FileSourceMap>) {
    let mut hir_file = HirFile::default();
    let mut source_map = FileSourceMap::default();

    let tree = db.parse(file_id);
    let mut lower_ctx = LowerFileCtx {
        db,
        file_id,
        file: &mut hir_file,
        file_source_map: &mut source_map,
        region_tree: RegionTreeBuilder::new(),
    };
    match tree.root() {
        Some(root) if ast::CompilationUnit::can_cast(root.kind()) => {
            lower_ctx.lower_file(ast::CompilationUnit::cast(root).unwrap());
        }
        Some(root) if ast::LibraryMap::can_cast(root.kind()) => {
            lower_ctx.lower_library_map(ast::LibraryMap::cast(root).unwrap());
        }
        _ => {}
    }

    hir_file.shrink_to_fit();
    source_map.shrink_to_fit();

    (Arc::new(hir_file), Arc::new(source_map))
}
