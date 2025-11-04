use la_arena::Idx;
use proc_macro_utils::define_container;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
    slang_ext::{AstNodeExt, PackageDeclaration},
};
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
    text_edit::TextRange,
};

use super::{
    Ident,
    aggregate::{ClassDef, ClassId, StructDef, StructId, lower_class_def, lower_struct_def},
    alloc_idx_and_src,
    block::{BlockInfo, LocalBlockId},
    declaration::{Declaration, DeclarationId, LowerDeclaration},
    expr::{Expr, LowerExpr, declarator::Declarator, timing_control::EventExpr},
    lower_ident, lower_ident_opt,
    proc::{LowerProc, LowerProcCtx, Proc, ProcId},
    stmt::{Stmt, StmtId, impl_lower_stmt},
    subroutine::{Subroutine, SubroutineId, SubroutineSrc, lower_subroutine},
    typedef::{Typedef, TypedefId, lower_typedef_data_ty},
};
use crate::{
    container::{ContainerId, InFile},
    db::{HirDb, InternDb},
    file::HirFileId,
    hir_def::Arena,
    region_tree::RegionTree,
    source_map::{IsNamedSrc, IsSrc, SourceMap, ToAstNode},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PackageImportMember {
    All,
    Named(Ident),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageImportItem {
    pub package: Ident,
    pub member: PackageImportMember,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageImport {
    pub items: SmallVec<[PackageImportItem; 2]>,
}

pub type PackageImportId = Idx<PackageImport>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageExportItem {
    pub package: Ident,
    pub member: PackageImportMember,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageExport {
    All,
    Items(SmallVec<[PackageExportItem; 2]>),
}

pub type PackageExportId = Idx<PackageExport>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageImportSrc {
    pub node: SyntaxNodePtr,
}

impl IsSrc for PackageImportSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for PackageImportSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        None
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        None
    }
}

impl<'a> ToAstNode<'a, ast::PackageImportDeclaration<'a>> for PackageImportSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::PackageImportDeclaration<'a>> {
        let mut node = self.node.to_node(tree)?;
        while !ast::PackageImportDeclaration::can_cast(node.kind()) {
            node = node.children().find_map(|elem| elem.as_node()).unwrap();
        }
        ast::PackageImportDeclaration::cast(node)
    }
}

impl From<ast::PackageImportDeclaration<'_>> for PackageImportSrc {
    fn from(node: ast::PackageImportDeclaration<'_>) -> Self {
        PackageImportSrc { node: AstNodeExt::to_ptr(&node) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageExportSrc {
    pub node: SyntaxNodePtr,
}

impl IsSrc for PackageExportSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for PackageExportSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        None
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        None
    }
}

impl<'a> ToAstNode<'a, syntax::ast::Member<'a>> for PackageExportSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<syntax::ast::Member<'a>> {
        let node = self.node.to_node(tree)?;
        syntax::ast::Member::cast(node)
    }
}

impl From<syntax::ast::Member<'_>> for PackageExportSrc {
    fn from(node: syntax::ast::Member<'_>) -> Self {
        PackageExportSrc { node: AstNodeExt::to_ptr(&node) }
    }
}

impl From<ast::PackageExportDeclaration<'_>> for PackageExportSrc {
    fn from(node: ast::PackageExportDeclaration<'_>) -> Self {
        PackageExportSrc { node: AstNodeExt::to_ptr(&node) }
    }
}

impl From<ast::PackageExportAllDeclaration<'_>> for PackageExportSrc {
    fn from(node: ast::PackageExportAllDeclaration<'_>) -> Self {
        PackageExportSrc { node: AstNodeExt::to_ptr(&node) }
    }
}

pub fn lower_package_import_item(item: ast::PackageImportItem) -> Option<PackageImportItem> {
    let package = lower_ident(item.package())?;

    let member = match item.item()?.kind() {
        TokenKind::STAR => PackageImportMember::All,
        _ => PackageImportMember::Named(lower_ident(item.item())?),
    };

    Some(PackageImportItem { package, member })
}

define_container! {
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct Package {
        name: Option<Ident>,

        declarations: [Declaration],
        typedefs: [Typedef],
        structs: [StructDef],
        classes: [ClassDef],
        procs: [Proc],
        subroutines: [Subroutine],
        exports: [PackageExport],

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
    pub struct PackageSourceMap {
        items: Vec<PackageItem>,
        region_tree: RegionTree,

        declaration_srcs: [Declaration | super::declaration::DeclarationSrc],
        typedef_srcs: [Typedef | super::typedef::TypedefSrc],
        struct_srcs: [StructDef | super::aggregate::StructSrc],
        class_srcs: [ClassDef | super::aggregate::ClassSrc],
        package_export_srcs: [PackageExport | PackageExportSrc],
        subroutine_srcs: [Subroutine | SubroutineSrc],

        proc_srcs: [Proc | super::proc::ProcSrc],

        expr_srcs: [Expr | super::expr::ExprSrc],
        event_expr_srcs: [EventExpr | super::expr::timing_control::EventExprSrc],
        decl_srcs: [Declarator | super::expr::declarator::DeclaratorSrc],
        stmt_srcs: [Stmt | super::stmt::StmtSrc] => {
            [StmtId | super::stmt::StmtSrc],
            [LocalBlockId | super::block::BlockSrc],
        },
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum PackageItem {
        DeclarationId,
        TypedefId,
        StructId,
        ClassId,
        ProcId,
        PackageExportId,
        SubroutineId,
    }
}

impl PackageSourceMap {
    pub(crate) fn ptr(&self, item: PackageItem) -> SyntaxNodePtr {
        match item {
            PackageItem::DeclarationId(idx) => self.declaration_srcs.get(idx).ptr(),
            PackageItem::TypedefId(idx) => self.typedef_srcs.get(idx).node,
            PackageItem::StructId(idx) => self.struct_srcs.get(idx).node,
            PackageItem::ClassId(idx) => self.class_srcs.get(idx).node,
            PackageItem::ProcId(idx) => self.proc_srcs.get(idx).0,
            PackageItem::PackageExportId(idx) => self.package_export_srcs.get(idx).node,
            PackageItem::SubroutineId(idx) => self.subroutine_srcs.get(idx).0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    pub name: Option<Ident>,
    pub parent: Option<LocalPackageId>,
}

pub type LocalPackageId = Idx<PackageInfo>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

impl IsSrc for PackageSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for PackageSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> ToAstNode<'a, PackageDeclaration<'a>> for PackageSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<PackageDeclaration<'a>> {
        let mut node = self.node.to_node(tree)?;
        while !PackageDeclaration::can_cast(node.kind()) {
            node = node.children().find_map(|elem| elem.as_node())?;
        }
        PackageDeclaration::cast(node)
    }
}

impl From<PackageDeclaration<'_>> for PackageSrc {
    fn from(node: PackageDeclaration<'_>) -> Self {
        let name = node.header().name().map(SyntaxTokenPtr::from_token);
        PackageSrc { node: AstNodeExt::to_ptr(&node), name }
    }
}

pub type PackageData = Arc<(Package, PackageSourceMap)>;
pub type PackageId = InFile<LocalPackageId>;

pub(crate) struct LowerPackageCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) package_id: PackageId,
    pub(crate) package: &'a mut Package,
    pub(crate) package_source_map: &'a mut PackageSourceMap,
    pub(crate) region_tree: crate::region_tree::RegionTreeBuilder,
}

use super::{
    declaration::impl_lower_declaration,
    expr::{declarator::impl_lower_decl, impl_lower_expr, timing_control::impl_lower_event_expr},
};

impl_lower_expr!(LowerPackageCtx<'_>, package, package_source_map);
impl_lower_decl!(LowerPackageCtx<'_>, package, package_source_map);
impl_lower_event_expr!(LowerPackageCtx<'_>, package, package_source_map);
impl_lower_stmt!(LowerPackageCtx<'_>, package_id, package, package_source_map);
impl_lower_declaration!(LowerPackageCtx<'_>, package, package_source_map);

impl LowerProc for LowerPackageCtx<'_> {
    fn proc_ctx(&mut self) -> LowerProcCtx<'_> {
        LowerProcCtx {
            db: self.db,
            file_id: self.file_id,
            cont_id: self.package_id.into(),

            procs: &mut self.package.procs,
            proc_srcs: &mut self.package_source_map.proc_srcs,

            stmts: &mut self.package.stmts,
            stmt_srcs: &mut self.package_source_map.stmt_srcs,

            exprs: &mut self.package.exprs,
            expr_srcs: &mut self.package_source_map.expr_srcs,

            event_exprs: &mut self.package.event_exprs,
            event_expr_srcs: &mut self.package_source_map.event_expr_srcs,

            decls: &mut self.package.decls,
            decl_srcs: &mut self.package_source_map.decl_srcs,
        }
    }
}

impl LowerPackageCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = ContainerId::PackageId(self.package_id);
        let struct_def = lower_struct_def(struct_ty.clone(), container_id, |ty| {
            self.expr_ctx().lower_data_ty(ty)
        });

        alloc_idx_and_src! {
            struct_def => self.package.structs,
            struct_ty => self.package_source_map.struct_srcs,
        }
    }

    fn lower_typedef(&mut self, typedef_decl: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef_decl.name());

        let typedef_id = alloc_idx_and_src! {
            Typedef { name, ty: None } => self.package.typedefs,
            typedef_decl => self.package_source_map.typedef_srcs,
        };

        let data_ty = typedef_decl.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            ContainerId::PackageId(self.package_id),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.expr_ctx().lower_data_ty(ty),
        );

        self.package.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_subroutine_decl(&mut self, func: ast::FunctionDeclaration) -> Option<SubroutineId> {
        let subroutine = lower_subroutine(&func, |ty| self.expr_ctx().lower_data_ty(ty))?;

        let subroutine_id = alloc_idx_and_src! {
            subroutine => self.package.subroutines,
            func => self.package_source_map.subroutine_srcs,
        };

        Some(subroutine_id)
    }

    fn lower_package_export(&mut self, export: ast::PackageExportDeclaration) -> PackageExportId {
        let mut items = SmallVec::<[PackageExportItem; 2]>::new();
        for item in export.items().children() {
            if let Some(lowered) = lower_package_import_item(item) {
                let PackageImportItem { package, member } = lowered;
                items.push(PackageExportItem { package, member });
            }
        }

        alloc_idx_and_src! {
            PackageExport::Items(items) => self.package.exports,
            export => self.package_source_map.package_export_srcs,
        }
    }

    fn lower_package_export_all(
        &mut self,
        export: ast::PackageExportAllDeclaration,
    ) -> PackageExportId {
        alloc_idx_and_src! {
            PackageExport::All => self.package.exports,
            export => self.package_source_map.package_export_srcs,
        }
    }

    fn lower_class_decl(&mut self, class_decl: ast::ClassDeclaration) -> ClassId {
        let container_id = ContainerId::PackageId(self.package_id);
        let class_def = lower_class_def(
            class_decl.clone(),
            container_id,
            |ty| self.expr_ctx().lower_data_ty(ty),
        );

        alloc_idx_and_src! {
            class_def => self.package.classes,
            class_decl => self.package_source_map.class_srcs,
        }
    }

    pub(crate) fn lower_package_decl(&mut self, decl: PackageDeclaration) {
        for member in decl.members().children() {
            let item = match member {
                ast::Member::DataDeclaration(data_decl) => {
                    Some(self.declaration_ctx().lower_data_decl(data_decl).into())
                }
                ast::Member::NetDeclaration(net_decl) => {
                    Some(self.declaration_ctx().lower_net_decl(net_decl).into())
                }
                ast::Member::ParameterDeclarationStatement(param_decl) => Some(
                    self.declaration_ctx().lower_param_decl_base(param_decl.parameter()).into(),
                ),
                ast::Member::TypedefDeclaration(typedef_decl) => {
                    Some(self.lower_typedef(typedef_decl).into())
                }
                ast::Member::ClassDeclaration(class_decl) => {
                    Some(self.lower_class_decl(class_decl).into())
                }
                ast::Member::ProceduralBlock(proc) => Some(self.proc_ctx().lower_proc(proc).into()),
                ast::Member::FunctionDeclaration(fn_decl) => {
                    self.lower_subroutine_decl(fn_decl).map(|sub_id| sub_id.into())
                }
                ast::Member::PackageExportDeclaration(export_decl) => {
                    Some(self.lower_package_export(export_decl).into())
                }
                ast::Member::PackageExportAllDeclaration(export_all_decl) => {
                    Some(self.lower_package_export_all(export_all_decl).into())
                }
                ast::Member::EmptyMember(_)
                | ast::Member::TimeUnitsDeclaration(_)
                | ast::Member::PackageImportDeclaration(_) => None,
                _ => None,
            };

            if let Some(item) = item {
                self.package_source_map.items.push(item);
            }

            self.region_tree.handle_node(member.syntax());
        }

        if let Some(end_token) = decl.endpackage() {
            self.region_tree.stage(Some(end_token));
        }

        self.package_source_map.region_tree = self.region_tree.finish();
    }
}

pub(crate) fn package_with_source_map_query(
    db: &dyn HirDb,
    package_id: PackageId,
) -> (Arc<Package>, Arc<PackageSourceMap>) {
    let local_package_id = package_id.value;
    let file_id = package_id.file_id;

    let (file, file_source_map) = db.hir_file_with_source_map(file_id);
    let tree = db.parse(file_id);

    let mut package =
        Package { name: file.packages.get(local_package_id).name.clone(), ..Default::default() };
    let mut package_source_map = PackageSourceMap::default();

    let src = file_source_map.package_srcs.get(local_package_id);
    if let Some(ast_package) = src.to_node(&tree) {
        let mut lower_ctx = LowerPackageCtx {
            db,
            file_id,
            package_id,
            package: &mut package,
            package_source_map: &mut package_source_map,
            region_tree: crate::region_tree::RegionTreeBuilder::new(),
        };
        lower_ctx.lower_package_decl(ast_package);
    }

    package.shrink_to_fit();
    package_source_map.shrink_to_fit();
    (Arc::new(package), Arc::new(package_source_map))
}

pub(crate) fn packages_by_name_query(db: &dyn HirDb) -> Arc<FxHashMap<Ident, Vec<PackageId>>> {
    let mut map: FxHashMap<Ident, Vec<PackageId>> = FxHashMap::default();

    for file_id in db.files().iter() {
        let file_id = HirFileId(*file_id);
        let hir_file = db.hir_file(file_id);

        for (local_package_id, package_info) in hir_file.packages.iter() {
            if let Some(name) = &package_info.name {
                map.entry(name.clone()).or_default().push(InFile::new(file_id, local_package_id));
            }
        }
    }

    for packages in map.values_mut() {
        packages
            .sort_by_key(|pkg_id| (pkg_id.file_id.file_id().0, pkg_id.value.into_raw().into_u32()));
        packages.dedup();
    }

    Arc::new(map)
}
