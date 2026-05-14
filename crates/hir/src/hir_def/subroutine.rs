use base_db::intern::Lookup;
use la_arena::{Arena, Idx};
use proc_macro_utils::define_container;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
    match_ast,
};
use triomphe::Arc;
use utils::get::Get;

use super::{
    Ident,
    aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
    alloc_idx_and_src,
    block::{BlockInfo, BlockItem, BlockSrc, LocalBlockId},
    declaration::{
        Declaration, DeclarationId, DeclarationSrc, LowerDeclaration, impl_lower_declaration,
    },
    expr::{
        Expr, ExprSrc, LowerExpr,
        data_ty::DataTy,
        declarator::{Declarator, DeclaratorSrc, impl_lower_decl},
        impl_lower_expr,
        timing_control::{EventExpr, EventExprSrc},
    },
    lower_ident, lower_ident_opt,
    opaque::{OpaqueItem, OpaqueItemSrc, OpaqueKind, lower_opaque_node},
    stmt::{LowerStmt, Stmt, StmtId, StmtSrc, impl_lower_stmt},
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::{ContainerId, InFile},
    db::{HirDb, InternDb},
    define_src_with_name,
    file::HirFileId,
    hir_def::{
        HirData,
        declaration::DataDecl,
        expr::{declarator::LowerDecl, timing_control::impl_lower_event_expr},
    },
    region_tree::{RegionTree, RegionTreeBuilder},
    source_map::SourceMap,
};

define_container! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct Subroutine {
        name: Option<Ident>,
        kind: SubroutineKind,
        ports: SmallVec<[SubroutinePort; 4]>,
        has_body: bool,
        declarations: [Declaration],
        typedefs: [Typedef],
        structs: [StructDef],
        opaque_items: [OpaqueItem],
        exprs: [Expr],
        event_exprs: [EventExpr],
        decls: [Declarator],
        stmts: [Stmt] => {
            [StmtId | Stmt],
            [LocalBlockId | BlockInfo],
        },
        source_map: SubroutineSourceMap
    }
}

impl Default for Subroutine {
    fn default() -> Self {
        Subroutine {
            name: None,
            kind: SubroutineKind::Task,
            ports: SmallVec::new(),
            has_body: false,
            declarations: Arena::new(),
            typedefs: Arena::new(),
            structs: Arena::new(),
            opaque_items: Arena::new(),
            exprs: Arena::new(),
            event_exprs: Arena::new(),
            decls: Arena::new(),
            stmts: Arena::new(),
            source_map: SubroutineSourceMap::default(),
        }
    }
}

define_container! {
    #[derive(Default, Debug, PartialEq, Eq, Clone)]
    pub struct SubroutineSourceMap {
        items: SmallVec<[BlockItem; 2]>,
        region_tree: RegionTree,

        declaration_srcs: [Declaration | DeclarationSrc],
        typedef_srcs: [Typedef | TypedefSrc],
        struct_srcs: [StructDef | StructSrc],
        opaque_srcs: [OpaqueItem | OpaqueItemSrc],
        expr_srcs: [Expr | ExprSrc],
        event_expr_srcs: [EventExpr | EventExprSrc],
        decl_srcs: [Declarator | DeclaratorSrc],
        stmt_srcs: [Stmt | StmtSrc] => {
            [StmtId | StmtSrc],
            [LocalBlockId | BlockSrc],
        },
        block_srcs: FxHashMap<BlockSrc, LocalBlockId>,
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SubroutineKind {
    Task,
    Function { return_ty: Option<DataTy> },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SubroutinePort {
    pub direction: SubroutinePortDir,
    pub ty: Option<DataTy>,
    pub name: Option<Ident>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct SubroutinePortId(pub u32);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum SubroutinePortDir {
    Input,
    Output,
    Inout,
    Ref,
    ConstRef,
    #[default]
    Unknown,
}

define_src_with_name!(SubroutineSrc(ast::FunctionDeclaration));

pub type LocalSubroutineId = Idx<Subroutine>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct SubroutineId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct SubroutineLoc {
    pub cont_id: ContainerId,
    pub src: InFile<SubroutineSrc>,
}

pub fn lower_subroutine<F>(func: &ast::FunctionDeclaration, mut lower_ty: F) -> Option<Subroutine>
where
    F: FnMut(ast::DataType) -> DataTy,
{
    let prototype = func.prototype();
    let name = lower_name(prototype.name())?;

    let is_task = func.as_task_declaration().is_some();

    let mut ports = SmallVec::<[SubroutinePort; 4]>::new();
    if let Some(port_list) = prototype.port_list() {
        for port_base in port_list.ports().children() {
            if let Some(port) = port_base.as_function_port() {
                let mut dir = map_direction(port.direction().map(|tok| tok.kind()));
                if matches!(dir, SubroutinePortDir::Ref) && port.const_keyword().is_some() {
                    dir = SubroutinePortDir::ConstRef;
                }

                let ty = port.data_type().map(&mut lower_ty);
                let name = lower_ident_opt(port.declarator().name());
                ports.push(SubroutinePort { direction: dir, ty, name });
            } else if port_base.as_default_function_port().is_some() {
                ports.push(SubroutinePort {
                    direction: SubroutinePortDir::Input,
                    ty: None,
                    name: None,
                });
            }
        }
    }

    let kind = if is_task {
        SubroutineKind::Task
    } else {
        let ret_ty = lower_ty(prototype.return_type());
        SubroutineKind::Function { return_ty: Some(ret_ty) }
    };

    Some(Subroutine { name: Some(name), kind, ports, ..Default::default() })
}

fn lower_name(name: ast::Name) -> Option<Ident> {
    if let Some(id) = name.as_identifier_name().and_then(|n| n.identifier()) {
        return lower_ident(Some(id));
    }
    if let Some(select) = name.as_identifier_select_name() {
        return select.identifier().and_then(|tok| lower_ident(Some(tok)));
    }
    if let Some(scoped) = name.as_scoped_name() {
        return lower_name(scoped.right());
    }
    None
}

fn map_direction(kind: Option<TokenKind>) -> SubroutinePortDir {
    match kind {
        Some(TokenKind::OUTPUT_KEYWORD) => SubroutinePortDir::Output,
        Some(TokenKind::IN_OUT_KEYWORD) => SubroutinePortDir::Inout,
        Some(TokenKind::REF_KEYWORD) => SubroutinePortDir::Ref,
        Some(TokenKind::INPUT_KEYWORD) | None => SubroutinePortDir::Input,
        Some(_) => SubroutinePortDir::Unknown,
    }
}

pub struct LowerSubroutineBodyCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) subroutine_id: SubroutineId,
    pub(crate) subroutine: &'a mut Subroutine,
    pub(crate) subroutine_source_map: &'a mut SubroutineSourceMap,
    pub(crate) region_tree: RegionTreeBuilder,
}

impl_lower_expr!(LowerSubroutineBodyCtx<'_>, subroutine, subroutine_source_map);
impl_lower_decl!(LowerSubroutineBodyCtx<'_>, subroutine, subroutine_source_map);
impl_lower_event_expr!(LowerSubroutineBodyCtx<'_>, subroutine, subroutine_source_map);
impl_lower_stmt!(LowerSubroutineBodyCtx<'_>, subroutine_id, subroutine, subroutine_source_map);
impl_lower_declaration!(LowerSubroutineBodyCtx<'_>, subroutine, subroutine_source_map);

impl LowerSubroutineBodyCtx<'_> {
    fn container_id(&self) -> ContainerId {
        ContainerId::SubroutineId(self.subroutine_id)
    }

    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = self.container_id();
        let struct_def =
            lower_struct_def(struct_ty, container_id, |ty| self.expr_ctx().lower_data_ty(ty));

        alloc_idx_and_src! {
            struct_def => self.subroutine.structs,
            struct_ty => self.subroutine_source_map.struct_srcs,
        }
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());

        let typedef_id = alloc_idx_and_src! {
            Typedef { name, ty: None } => self.subroutine.typedefs,
            typedef => self.subroutine_source_map.typedef_srcs,
        };

        let data_ty = typedef.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            self.container_id(),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.expr_ctx().lower_data_ty(ty),
        );

        self.subroutine.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_local_variable_decl(
        &mut self,
        local_decl: ast::LocalVariableDeclaration,
    ) -> DeclarationId {
        let const_kw = false;
        let var_kw = local_decl.var().is_some();
        let ty = self.expr_ctx().lower_data_ty(local_decl.type_());

        let parent = self.subroutine.declarations.nxt_idx().into();
        let decls = self.decl_ctx().lower_declarators(local_decl.declarators(), parent);

        alloc_idx_and_src! {
            DataDecl { ty, const_kw, var_kw, decls } => self.subroutine.declarations,
            local_decl => self.subroutine_source_map.declaration_srcs,
        }
    }

    pub(crate) fn lower_items(&mut self, func: ast::FunctionDeclaration) {
        self.subroutine.has_body = true;

        for item in func.items().children() {
            self.region_tree.handle_node(item.syntax());

            let syntax = item.syntax();
            match_ast! { syntax,
                ast::Statement[it] => {
                    let stmt_id = self.stmt_ctx().lower_stmt(it);
                    if let Some(block_stmt) = it.as_block_statement() {
                        let block_src = BlockSrc::from(block_stmt);
                        let local_block_id = LocalBlockId(stmt_id);
                        self.subroutine_source_map.block_srcs.insert(block_src, local_block_id);
                    }
                    self.subroutine_source_map.items.push(BlockItem::StmtId(stmt_id));
                },
                ast::DataDeclaration[it] => {
                    let decl_id = self.declaration_ctx().lower_data_decl(it);
                    self.subroutine_source_map.items.push(BlockItem::DeclarationId(decl_id));
                },
                ast::LocalVariableDeclaration[it] => {
                    let decl_id = self.lower_local_variable_decl(it);
                    self.subroutine_source_map.items.push(BlockItem::DeclarationId(decl_id));
                },
                ast::TypedefDeclaration[it] => {
                    let typedef_id = self.lower_typedef(it);
                    self.subroutine_source_map.items.push(BlockItem::TypedefId(typedef_id));
                },
                _ => {
                    let (opaque, src) =
                        lower_opaque_node(item.syntax(), None, OpaqueKind::BlockItem);
                    let opaque_id = self.subroutine.opaque_items.alloc(opaque);
                    self.subroutine_source_map.opaque_srcs.insert(src, opaque_id);
                    self.subroutine_source_map.items.push(BlockItem::OpaqueItemId(opaque_id));
                },
            }
        }

        self.region_tree.stage(func.end());
        self.subroutine_source_map.region_tree = self.region_tree.finish();
    }
}

pub fn lower_subroutine_body(ctx: &mut LowerSubroutineBodyCtx<'_>, func: ast::FunctionDeclaration) {
    ctx.lower_items(func);
}

pub(crate) fn subroutine_with_source_map_query(
    db: &dyn HirDb,
    subroutine_id: SubroutineId,
) -> (Arc<Subroutine>, Arc<SubroutineSourceMap>) {
    let SubroutineLoc { cont_id, src } = subroutine_id.lookup(db);

    match cont_id {
        ContainerId::HirFileId(file_id) => {
            let file = db.hir_file(file_id);
            let (_, file_src_map) = db.hir_file_with_source_map(file_id);
            let local_id = file_src_map.get(src.value);
            let subroutine = file.subroutines[local_id].clone();
            let source_map =
                file.subroutine_source_maps.get(&local_id).cloned().unwrap_or_default();
            (Arc::new(subroutine), Arc::new(source_map))
        }
        ContainerId::ModuleId(module_id) => {
            let module = db.module(module_id);
            let (_, module_src_map) = db.module_with_source_map(module_id);
            let local_id = module_src_map.get(src.value);
            let subroutine = module.subroutines[local_id].clone();
            let source_map =
                module.subroutine_source_maps.get(&local_id).cloned().unwrap_or_default();
            (Arc::new(subroutine), Arc::new(source_map))
        }
        _ => unreachable!("subroutine parent must be file or module"),
    }
}
