use la_arena::Arena;
use proc_macro_utils::define_container;
use smallvec::SmallVec;
use syntax::{
    ast::{self, AstNode},
    ptr::SyntaxNodePtr,
    slang_ext::PackageDeclaration,
};
use triomphe::Arc;
use utils::{define_enum_deriving_from, get::Get};

use super::{
    aggregate::{
        ClassDef, ClassId, ClassSrc, StructDef, StructId, StructSrc, lower_class_def,
        lower_struct_def,
    },
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
    package::{
        LocalPackageId, PackageImport, PackageImportId, PackageImportItem, PackageImportSrc,
        PackageInfo, PackageSrc, lower_package_import_item,
    },
    proc::{LowerProc, LowerProcCtx, Proc, ProcId, ProcSrc},
    stmt::{Stmt, StmtId, StmtSrc, impl_lower_stmt},
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::ContainerId,
    db::{HirDb, InternDb},
    file::HirFileId,
    hir_def::lower_ident_opt,
    region_tree::{RegionTree, RegionTreeBuilder},
    source_map::SourceMap,
};

define_container! {
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct HirFile {
        modules: [ModuleInfo],
        packages: [PackageInfo],
        procs: [Proc],

        typedefs: [Typedef],
        structs: [StructDef],
        classes: [ClassDef],
        package_imports: [PackageImport],

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
        package_srcs: [PackageInfo | PackageSrc],
        proc_srcs: [Proc | ProcSrc],

        declaration_srcs: [Declaration | DeclarationSrc],
        typedef_srcs: [Typedef | TypedefSrc],
        struct_srcs: [StructDef | StructSrc],
        class_srcs: [ClassDef | ClassSrc],
        package_import_srcs: [PackageImport | PackageImportSrc],
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
        LocalModuleId,
        LocalPackageId,
        ProcId,
        DeclarationId,
        TypedefId,
        StructId,
        ClassId,
        PackageImportId,
    }
}

impl FileSourceMap {
    pub fn item_to_ptr(&self, item: &FileItem) -> SyntaxNodePtr {
        match item {
            FileItem::LocalModuleId(idx) => self.get(*idx).node,
            FileItem::LocalPackageId(idx) => self.get(*idx).node,
            FileItem::ProcId(idx) => self.get(*idx).0,
            FileItem::DeclarationId(idx) => self.get(*idx).ptr(),
            FileItem::TypedefId(idx) => self.get(*idx).ptr(),
            FileItem::StructId(idx) => self.get(*idx).node,
            FileItem::ClassId(idx) => self.get(*idx).node,
            FileItem::PackageImportId(idx) => self.get(*idx).node,
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
        let struct_def = lower_struct_def(struct_ty.clone(), container_id, |ty| {
            self.expr_ctx().lower_data_ty(ty)
        });

        alloc_idx_and_src! {
            struct_def => self.file.structs,
            struct_ty => self.file_source_map.struct_srcs,
        }
    }

    fn lower_class_decl(&mut self, class_decl: ast::ClassDeclaration) -> ClassId {
        let container_id = ContainerId::HirFileId(self.file_id);
        let class_def = lower_class_def(
            class_decl.clone(),
            container_id,
            |ty| self.expr_ctx().lower_data_ty(ty),
        );

        alloc_idx_and_src! {
            class_def => self.file.classes,
            class_decl => self.file_source_map.class_srcs,
        }
    }

    fn lower_package_import(&mut self, import: ast::PackageImportDeclaration) -> PackageImportId {
        let mut items = SmallVec::<[PackageImportItem; 2]>::new();
        for item in import.items().children() {
            if let Some(lowered) = lower_package_import_item(item) {
                items.push(lowered);
            }
        }

        alloc_idx_and_src! {
            PackageImport { items } => self.file.package_imports,
            import => self.file_source_map.package_import_srcs,
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

    pub(crate) fn lower_file(&mut self, root: ast::CompilationUnit) {
        for member in root.members().children() {
            use ast::Member::*;
            let idx = match member {
                ModuleDeclaration(decl) => {
                    if let Some(package_decl) = PackageDeclaration::from_module(decl) {
                        self.register_package_decl(package_decl, None).into()
                    } else {
                        let name = lower_ident_opt(decl.header().name());

                        alloc_idx_and_src! {
                            ModuleInfo { name } => self.file.modules,
                            decl => self.file_source_map.module_srcs,
                        }
                        .into()
                    }
                }
                ProceduralBlock(proc) => self.proc_ctx().lower_proc(proc).into(),
                DataDeclaration(data_decl) => {
                    self.declaration_ctx().lower_data_decl(data_decl).into()
                }
                NetDeclaration(net_decl) => self.declaration_ctx().lower_net_decl(net_decl).into(),
                PackageImportDeclaration(import_decl) => {
                    self.lower_package_import(import_decl).into()
                }
                ClassDeclaration(class_decl) => self.lower_class_decl(class_decl).into(),
                EmptyMember(_x) => continue,
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                _ => unimplemented!("{:?}", member.syntax().kind()),
            };
            self.file_source_map.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }

        self.region_tree.stage(root.end_of_file());
        self.file_source_map.region_tree = self.region_tree.finish();
    }
}

impl LowerFileCtx<'_> {
    fn register_package_decl(
        &mut self,
        package_decl: PackageDeclaration,
        parent: Option<LocalPackageId>,
    ) -> LocalPackageId {
        let name = lower_ident_opt(package_decl.header().name());
        let local_package_id = alloc_idx_and_src! {
            PackageInfo { name, parent } => self.file.packages,
            package_decl => self.file_source_map.package_srcs,
        };

        for member in package_decl.members().children() {
            if let ast::Member::ModuleDeclaration(module_decl) = member
                && let Some(nested_pkg) = PackageDeclaration::from_module(module_decl)
            {
                self.register_package_decl(nested_pkg, Some(local_package_id));
            }
        }

        local_package_id
    }
}

pub(crate) fn hir_file_with_source_map_query(
    db: &dyn HirDb,
    file_id: HirFileId,
) -> (Arc<HirFile>, Arc<FileSourceMap>) {
    let mut hir_file = HirFile::default();
    let mut source_map = FileSourceMap::default();

    let tree = db.parse(file_id);
    let Some(root) = tree.root().and_then(ast::CompilationUnit::cast) else {
        return (Arc::new(hir_file), Arc::new(source_map));
    };

    let mut lower_ctx = LowerFileCtx {
        db,
        file_id,
        file: &mut hir_file,
        file_source_map: &mut source_map,
        region_tree: RegionTreeBuilder::new(),
    };
    lower_ctx.lower_file(root);

    hir_file.shrink_to_fit();
    source_map.shrink_to_fit();

    (Arc::new(hir_file), Arc::new(source_map))
}
