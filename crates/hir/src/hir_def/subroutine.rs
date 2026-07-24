use la_arena::{Arena, Idx};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
    match_ast,
};
use triomphe::Arc;

use super::{
    Ident,
    aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
    alloc_with_source,
    block::{BlockInfo, BlockItem, BlockSrc, LocalBlockId},
    declaration::{DataDecl, Declaration, DeclarationId, DeclarationSrc},
    expr::{
        Expr, ExprId, ExprSrc,
        data_ty::DataTy,
        declarator::{DeclId, Declarator, DeclaratorSrc, empty_decls_range},
        timing_control::{EventExpr, EventExprId, EventExprSrc},
    },
    lower::{LoweringCtx, SubroutineStore},
    lower_ident_opt,
    stmt::{Stmt, StmtId, StmtSrc},
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::{InContainer, ScopeId},
    db::HirDb,
    region_tree::RegionTree,
    source_map::{AstKind, NamedAstId, SourceMap},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Subroutine {
    pub name: Option<Ident>,
    pub kind: SubroutineKind,
    pub ports: SmallVec<[SubroutinePort; 4]>,
    pub has_body: bool,
    pub declarations: Arena<Declaration>,
    pub typedefs: Arena<Typedef>,
    pub structs: Arena<StructDef>,
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
    pub stmts: Arena<Stmt>,
    pub source_map: SubroutineSourceMap,
}

impl Subroutine {
    pub fn shrink_to_fit(&mut self) {
        self.declarations.shrink_to_fit();
        self.typedefs.shrink_to_fit();
        self.structs.shrink_to_fit();
        self.exprs.shrink_to_fit();
        self.event_exprs.shrink_to_fit();
        self.decls.shrink_to_fit();
        self.stmts.shrink_to_fit();
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
            exprs: Arena::new(),
            event_exprs: Arena::new(),
            decls: Arena::new(),
            stmts: Arena::new(),
            source_map: SubroutineSourceMap::default(),
        }
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct SubroutineSourceMap {
    pub items: SmallVec<[BlockItem; 2]>,
    pub region_tree: RegionTree,
    pub declaration_srcs: SourceMap<DeclarationSrc, Declaration>,
    pub typedef_srcs: SourceMap<TypedefSrc, Typedef>,
    pub struct_srcs: SourceMap<StructSrc, StructDef>,
    pub expr_srcs: SourceMap<ExprSrc, Expr>,
    pub event_expr_srcs: SourceMap<EventExprSrc, EventExpr>,
    pub decl_srcs: SourceMap<DeclaratorSrc, Declarator>,
    pub stmt_srcs: SourceMap<StmtSrc, Stmt>,
    pub block_srcs: FxHashMap<BlockSrc, LocalBlockId>,
}

impl SubroutineSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.declaration_srcs.shrink_to_fit();
        self.typedef_srcs.shrink_to_fit();
        self.struct_srcs.shrink_to_fit();
        self.expr_srcs.shrink_to_fit();
        self.event_expr_srcs.shrink_to_fit();
        self.decl_srcs.shrink_to_fit();
        self.stmt_srcs.shrink_to_fit();
    }
}

crate::hir_def::impl_arena_getters!(
    Subroutine;
    DeclarationId => declarations => Declaration,
    TypedefId => typedefs => Typedef,
    StructId => structs => StructDef,
    ExprId => exprs => Expr,
    EventExprId => event_exprs => EventExpr,
    DeclId => decls => Declarator,
    StmtId => stmts => Stmt,
    LocalBlockId => stmts => BlockInfo,
);

crate::hir_def::impl_source_map_getters!(
    SubroutineSourceMap;
    DeclarationSrc => DeclarationId => declaration_srcs,
    TypedefSrc => TypedefId => typedef_srcs,
    StructSrc => StructId => struct_srcs,
    ExprSrc => ExprId => expr_srcs,
    EventExprSrc => EventExprId => event_expr_srcs,
    DeclaratorSrc => DeclId => decl_srcs,
    StmtSrc => StmtId => stmt_srcs,
    BlockSrc => LocalBlockId => stmt_srcs,
);

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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct FunctionDeclarationAst;

impl AstKind for FunctionDeclarationAst {
    type Node<'a> = ast::FunctionDeclaration<'a>;
}

pub type SubroutineSrc = NamedAstId<FunctionDeclarationAst>;

pub type LocalSubroutineId = Idx<Subroutine>;

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
        return lower_ident_opt(Some(id));
    }
    if let Some(select) = name.as_identifier_select_name() {
        return select.identifier().and_then(|tok| lower_ident_opt(Some(tok)));
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

pub(crate) type LowerSubroutineBodyCtx<'a> = LoweringCtx<'a, SubroutineStore<'a>>;

impl LowerSubroutineBodyCtx<'_> {
    fn container_id(&self) -> ScopeId {
        self.owner
    }

    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = self.container_id();
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
            self.container_id(),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.lower_data_ty(ty),
        );

        self.store.data.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_local_variable_decl(
        &mut self,
        local_decl: ast::LocalVariableDeclaration,
    ) -> DeclarationId {
        let const_kw = false;
        let var_kw = local_decl.var().is_some();
        let ty = self.lower_data_ty(local_decl.type_());

        let parent = self.alloc_declaration(
            DataDecl { ty, const_kw, var_kw, decls: empty_decls_range() },
            local_decl,
        );
        let decls = self.lower_declarators(local_decl.declarators(), parent.into());
        self.finish_declaration_decls(parent, decls);
        parent
    }

    pub(crate) fn lower_items(&mut self, func: ast::FunctionDeclaration) {
        self.store.data.has_body = true;

        for item in func.items().children() {
            self.region_tree.handle_node(item.syntax());

            let syntax = item.syntax();
            match_ast! { syntax,
                ast::Statement[it] => {
                    let stmt_id = self.lower_stmt(it);
                    if let Some(block_stmt) = it.as_block_statement() {
                        let block_src = BlockSrc::from_ast(self.file_id, block_stmt);
                        let local_block_id = LocalBlockId(stmt_id);
                        self.store.sources.block_srcs.insert(block_src, local_block_id);
                    }
                    self.store.sources.items.push(BlockItem::StmtId(stmt_id));
                },
                ast::DataDeclaration[it] => {
                    let decl_id = self.lower_data_decl(it);
                    self.store.sources.items.push(BlockItem::DeclarationId(decl_id));
                },
                ast::PortDeclaration[it] => {
                    if let Some(decl_id) =
                        self.lower_port_decl_as_data_decl(it)
                    {
                        self.store.sources.items.push(BlockItem::DeclarationId(decl_id));
                    }
                },
                ast::LocalVariableDeclaration[it] => {
                    let decl_id = self.lower_local_variable_decl(it);
                    self.store.sources.items.push(BlockItem::DeclarationId(decl_id));
                },
                ast::ParameterDeclarationStatement[it] => {
                    let decl_id = self.lower_param_decl_base(it.parameter());
                    self.store.sources.items.push(BlockItem::DeclarationId(decl_id));
                },
                ast::TypedefDeclaration[it] => {
                    let typedef_id = self.lower_typedef(it);
                    self.store.sources.items.push(BlockItem::TypedefId(typedef_id));
                },
                _ => {},
            }
        }

        self.region_tree.stage(func.end(), func.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }
}

pub(crate) fn lower_subroutine_body(
    ctx: &mut LowerSubroutineBodyCtx<'_>,
    func: ast::FunctionDeclaration,
) {
    ctx.lower_items(func);
}

pub(crate) fn subroutine_with_source_map_query(
    db: &dyn HirDb,
    subroutine_id: InContainer<LocalSubroutineId>,
) -> (Arc<Subroutine>, Arc<SubroutineSourceMap>) {
    match subroutine_id.cont_id {
        ScopeId::File(file_id) => {
            let file = db.hir_file(file_id);
            let subroutine = file.subroutines[subroutine_id.value].clone();
            let source_map = subroutine.source_map.clone();
            (Arc::new(subroutine), Arc::new(source_map))
        }
        ScopeId::Module(module_id) => {
            let module = db.module(module_id);
            let subroutine = module.subroutines[subroutine_id.value].clone();
            let source_map = subroutine.source_map.clone();
            (Arc::new(subroutine), Arc::new(source_map))
        }
        ScopeId::GenerateBlock(generate_block_id) => {
            let generate_block = db.generate_block(generate_block_id);
            let subroutine = generate_block.subroutines[subroutine_id.value].clone();
            let source_map = subroutine.source_map.clone();
            (Arc::new(subroutine), Arc::new(source_map))
        }
        ScopeId::Block(_)
        | ScopeId::Subroutine(_)
        | ScopeId::ClockingBlock(_)
        | ScopeId::Checker(_)
        | ScopeId::Covergroup(_) => {
            unreachable!("subroutines are lowered only in file, module, or generate-block scopes")
        }
    }
}
