use la_arena::{Arena, Idx};
use smallvec::SmallVec;
use syntax::{
    SyntaxToken, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
};
use triomphe::Arc;
use utils::define_enum_deriving_from;

use super::{
    LowerModuleCtx,
    continuous_assgin::{ContAssign, ContAssignId, ContAssignSrc},
    defparam::{DefParam, DefParamId, DefParamSrc},
    instantiation::{
        Instance, InstanceId, InstanceSrc, Instantiation, InstantiationId, InstantiationSrc,
        ParamAssign, ParamAssignId, ParamAssignSrc, PortConn, PortConnId, PortConnSrc,
    },
};
use crate::{
    base_db::intern::Lookup,
    container::{ArenaOwnerId, InFile, SubroutineParent, SubroutineScope},
    db::HirDb,
    hir_def::{
        Ident,
        aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
        alloc_with_optional_source_entry, alloc_with_source,
        declaration::{Declaration, DeclarationId, DeclarationSrc},
        expr::{
            Expr, ExprId, ExprSrc,
            declarator::{DeclId, Declarator, DeclaratorSrc},
            timing_control::{EventExpr, EventExprId, EventExprSrc},
        },
        lower::{GenerateBlockStore, LoweringCtx, SubroutineStore},
        lower_ident_opt,
        proc::{Proc, ProcId, ProcSrc},
        stmt::{Stmt, StmtId, StmtSrc},
        subroutine::{
            LocalSubroutineId, Subroutine, SubroutineSrc, lower_subroutine, lower_subroutine_body,
        },
        typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
    },
    region_tree::RegionTree,
    source_map::{
        FromSourceAst, IsNamedSrc, IsSrc, SourceAst, SourceMap, ToAstNode, root_token_in,
    },
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GenerateRegion {
    pub items: SmallVec<[GenerateItem; 4]>,
}

pub type GenerateRegionId = Idx<GenerateRegion>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum GenerateRegionSrc {
    GenerateRegion(SyntaxNodePtr),
    DirectItem(SyntaxNodePtr),
}

impl GenerateRegionSrc {
    /// Source-map key for synthetic generate regions that wrap a direct member.
    ///
    /// Direct generate regions do not have their own AST node, so use the
    /// member as the navigable location when that member belongs to the
    /// parsed root file. Include-expanded members still lower into HIR, but
    /// have no source entry here.
    pub fn from_direct_member(member: &ast::Member<'_>) -> Option<Self> {
        member
            .syntax()
            .text_range()
            .map(|_| Self::DirectItem(syntax::slang_ext::AstNodeExt::to_ptr(member)))
    }

    fn ptr(&self) -> SyntaxNodePtr {
        match self {
            GenerateRegionSrc::GenerateRegion(ptr) | GenerateRegionSrc::DirectItem(ptr) => *ptr,
        }
    }
}

impl IsSrc for GenerateRegionSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        self.ptr().kind()
    }

    fn range(&self) -> utils::text_edit::TextRange {
        self.ptr().range()
    }
}

impl IsNamedSrc for GenerateRegionSrc {
    fn name_kind(&self) -> Option<TokenKind> {
        None
    }

    fn name_range(&self) -> Option<utils::text_edit::TextRange> {
        None
    }
}

impl<'a> ToAstNode<'a, ast::GenerateRegion<'a>> for GenerateRegionSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::GenerateRegion<'a>> {
        match self {
            GenerateRegionSrc::GenerateRegion(ptr) => {
                let mut node = ptr.to_node(tree)?;
                while !ast::GenerateRegion::can_cast(node.kind()) && node.child_count() == 1 {
                    node = node.child_node(0)?;
                }
                ast::GenerateRegion::cast(node)
            }
            GenerateRegionSrc::DirectItem(_) => None,
        }
    }
}

impl From<ast::GenerateRegion<'_>> for GenerateRegionSrc {
    fn from(region: ast::GenerateRegion<'_>) -> Self {
        Self::GenerateRegion(syntax::slang_ext::AstNodeExt::to_ptr(&region))
    }
}

impl<'a> FromSourceAst<'a, ast::GenerateRegion<'a>> for GenerateRegionSrc {
    fn from_source_ast(region: SourceAst<ast::GenerateRegion<'a>>) -> Self {
        Self::GenerateRegion(syntax::slang_ext::AstNodeExt::to_ptr(&region.into_inner()))
    }
}

impl From<GenerateRegionSrc> for SyntaxNodePtr {
    fn from(src: GenerateRegionSrc) -> Self {
        src.ptr()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum GenerateBlockSrc {
    GenerateBlock { node: SyntaxNodePtr, name: Option<SyntaxTokenPtr> },
    LoopGenerate { node: SyntaxNodePtr, name: Option<SyntaxTokenPtr> },
    SingleMember { node: SyntaxNodePtr, name: Option<SyntaxTokenPtr> },
}

impl GenerateBlockSrc {
    pub fn from_generate_block(block: ast::GenerateBlock<'_>) -> Self {
        if let Some(parent) = block.syntax().parent()
            && let Some(loop_generate) = ast::LoopGenerate::cast(parent)
        {
            return loop_generate.into();
        }

        block.into()
    }

    pub fn node(&self) -> SyntaxNodePtr {
        match self {
            GenerateBlockSrc::GenerateBlock { node, .. }
            | GenerateBlockSrc::LoopGenerate { node, .. }
            | GenerateBlockSrc::SingleMember { node, .. } => *node,
        }
    }

    fn name(&self) -> Option<SyntaxTokenPtr> {
        match self {
            GenerateBlockSrc::GenerateBlock { name, .. }
            | GenerateBlockSrc::LoopGenerate { name, .. }
            | GenerateBlockSrc::SingleMember { name, .. } => *name,
        }
    }

    fn to_member<'a>(self, tree: &'a syntax::SyntaxTree) -> Option<ast::Member<'a>> {
        ast::Member::cast(self.node().to_node(tree)?)
    }
}

impl IsSrc for GenerateBlockSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        self.node().kind()
    }

    fn range(&self) -> utils::text_edit::TextRange {
        self.node().range()
    }
}

impl IsNamedSrc for GenerateBlockSrc {
    fn name_kind(&self) -> Option<TokenKind> {
        self.name().map(|name| name.kind())
    }

    fn name_range(&self) -> Option<utils::text_edit::TextRange> {
        self.name().map(|name| name.range())
    }
}

impl<'a> ToAstNode<'a, ast::GenerateBlock<'a>> for GenerateBlockSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::GenerateBlock<'a>> {
        match self {
            GenerateBlockSrc::GenerateBlock { node, .. } => {
                let mut node = node.to_node(tree)?;
                while !ast::GenerateBlock::can_cast(node.kind()) && node.child_count() == 1 {
                    node = node.child_node(0)?;
                }
                ast::GenerateBlock::cast(node)
            }
            GenerateBlockSrc::LoopGenerate { node, .. } => {
                let mut node = node.to_node(tree)?;
                while !ast::LoopGenerate::can_cast(node.kind()) && node.child_count() == 1 {
                    node = node.child_node(0)?;
                }
                let loop_generate = ast::LoopGenerate::cast(node)?;
                loop_generate.block().as_generate_block()
            }
            GenerateBlockSrc::SingleMember { .. } => None,
        }
    }
}

impl<'a> ToAstNode<'a, ast::LoopGenerate<'a>> for GenerateBlockSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::LoopGenerate<'a>> {
        match self {
            GenerateBlockSrc::LoopGenerate { node, .. } => {
                let mut node = node.to_node(tree)?;
                while !ast::LoopGenerate::can_cast(node.kind()) && node.child_count() == 1 {
                    node = node.child_node(0)?;
                }
                ast::LoopGenerate::cast(node)
            }
            GenerateBlockSrc::GenerateBlock { .. } => None,
            GenerateBlockSrc::SingleMember { .. } => None,
        }
    }
}

impl From<ast::GenerateBlock<'_>> for GenerateBlockSrc {
    fn from(block: ast::GenerateBlock<'_>) -> Self {
        let syntax = block.syntax();
        GenerateBlockSrc::GenerateBlock {
            node: syntax::slang_ext::AstNodeExt::to_ptr(&block),
            name: generate_block_name(block)
                .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token)),
        }
    }
}

impl From<ast::LoopGenerate<'_>> for GenerateBlockSrc {
    fn from(loop_generate: ast::LoopGenerate<'_>) -> Self {
        let block = loop_generate.block().as_generate_block();
        GenerateBlockSrc::LoopGenerate {
            node: syntax::slang_ext::AstNodeExt::to_ptr(&loop_generate),
            name: block.and_then(|block| {
                generate_block_name(block).and_then(|name| {
                    root_token_in(block.syntax(), name).map(SyntaxTokenPtr::from_token)
                })
            }),
        }
    }
}

impl From<ast::Member<'_>> for GenerateBlockSrc {
    fn from(member: ast::Member<'_>) -> Self {
        GenerateBlockSrc::SingleMember {
            node: syntax::slang_ext::AstNodeExt::to_ptr(&member),
            name: None,
        }
    }
}

impl From<GenerateBlockSrc> for SyntaxNodePtr {
    fn from(src: GenerateBlockSrc) -> Self {
        src.node()
    }
}

fn generate_block_name(block: ast::GenerateBlock<'_>) -> Option<SyntaxToken<'_>> {
    block
        .label()
        .and_then(|label| label.name())
        .or_else(|| block.begin_name().and_then(|name| name.name()))
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct GenerateBlock {
    pub name: Option<Ident>,
    pub kind: GenerateBlockKind,
    pub items: Vec<GenerateBlockItem>,
    pub region_tree: RegionTree,
    pub cont_assigns: Arena<ContAssign>,
    pub defparams: Arena<DefParam>,
    pub declarations: Arena<Declaration>,
    pub typedefs: Arena<Typedef>,
    pub structs: Arena<StructDef>,
    pub subroutines: Arena<Subroutine>,
    pub instantiations: Arena<Instantiation>,
    pub inst_param_assigns: Arena<ParamAssign>,
    pub instances: Arena<Instance>,
    pub inst_port_conns: Arena<PortConn>,
    pub procs: Arena<Proc>,
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
    pub stmts: Arena<Stmt>,
}

impl GenerateBlock {
    pub fn shrink_to_fit(&mut self) {
        self.cont_assigns.shrink_to_fit();
        self.defparams.shrink_to_fit();
        self.declarations.shrink_to_fit();
        self.typedefs.shrink_to_fit();
        self.structs.shrink_to_fit();
        self.subroutines.shrink_to_fit();
        self.instantiations.shrink_to_fit();
        self.inst_param_assigns.shrink_to_fit();
        self.instances.shrink_to_fit();
        self.inst_port_conns.shrink_to_fit();
        self.procs.shrink_to_fit();
        self.exprs.shrink_to_fit();
        self.event_exprs.shrink_to_fit();
        self.decls.shrink_to_fit();
        self.stmts.shrink_to_fit();
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct GenerateBlockSourceMap {
    pub items: Vec<GenerateBlockItem>,
    pub region_tree: RegionTree,
    pub assign_srcs: SourceMap<ContAssignSrc, ContAssign>,
    pub defparam_srcs: SourceMap<DefParamSrc, DefParam>,
    pub declaration_srcs: SourceMap<DeclarationSrc, Declaration>,
    pub typedef_srcs: SourceMap<TypedefSrc, Typedef>,
    pub struct_srcs: SourceMap<StructSrc, StructDef>,
    pub subroutine_srcs: SourceMap<SubroutineSrc, Subroutine>,
    pub instantiation_srcs: SourceMap<InstantiationSrc, Instantiation>,
    pub inst_param_assign_srcs: SourceMap<ParamAssignSrc, ParamAssign>,
    pub instance_srcs: SourceMap<InstanceSrc, Instance>,
    pub inst_port_conn_srcs: SourceMap<PortConnSrc, PortConn>,
    pub proc_srcs: SourceMap<ProcSrc, Proc>,
    pub expr_srcs: SourceMap<ExprSrc, Expr>,
    pub event_expr_srcs: SourceMap<EventExprSrc, EventExpr>,
    pub decl_srcs: SourceMap<DeclaratorSrc, Declarator>,
    pub stmt_srcs: SourceMap<StmtSrc, Stmt>,
}

impl GenerateBlockSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.assign_srcs.shrink_to_fit();
        self.defparam_srcs.shrink_to_fit();
        self.declaration_srcs.shrink_to_fit();
        self.typedef_srcs.shrink_to_fit();
        self.struct_srcs.shrink_to_fit();
        self.subroutine_srcs.shrink_to_fit();
        self.instantiation_srcs.shrink_to_fit();
        self.inst_param_assign_srcs.shrink_to_fit();
        self.instance_srcs.shrink_to_fit();
        self.inst_port_conn_srcs.shrink_to_fit();
        self.proc_srcs.shrink_to_fit();
        self.expr_srcs.shrink_to_fit();
        self.event_expr_srcs.shrink_to_fit();
        self.decl_srcs.shrink_to_fit();
        self.stmt_srcs.shrink_to_fit();
    }
}

crate::hir_def::impl_arena_getters!(
    GenerateBlock;
    ContAssignId => cont_assigns => ContAssign,
    DefParamId => defparams => DefParam,
    DeclarationId => declarations => Declaration,
    TypedefId => typedefs => Typedef,
    StructId => structs => StructDef,
    LocalSubroutineId => subroutines => Subroutine,
    InstantiationId => instantiations => Instantiation,
    ParamAssignId => inst_param_assigns => ParamAssign,
    InstanceId => instances => Instance,
    PortConnId => inst_port_conns => PortConn,
    ProcId => procs => Proc,
    ExprId => exprs => Expr,
    EventExprId => event_exprs => EventExpr,
    DeclId => decls => Declarator,
    StmtId => stmts => Stmt,
    crate::hir_def::block::LocalBlockId => stmts => crate::hir_def::block::BlockInfo,
);

crate::hir_def::impl_source_map_getters!(
    GenerateBlockSourceMap;
    ContAssignSrc => ContAssignId => assign_srcs,
    DefParamSrc => DefParamId => defparam_srcs,
    DeclarationSrc => DeclarationId => declaration_srcs,
    TypedefSrc => TypedefId => typedef_srcs,
    StructSrc => StructId => struct_srcs,
    SubroutineSrc => LocalSubroutineId => subroutine_srcs,
    InstantiationSrc => InstantiationId => instantiation_srcs,
    ParamAssignSrc => ParamAssignId => inst_param_assign_srcs,
    InstanceSrc => InstanceId => instance_srcs,
    PortConnSrc => PortConnId => inst_port_conn_srcs,
    ProcSrc => ProcId => proc_srcs,
    ExprSrc => ExprId => expr_srcs,
    EventExprSrc => EventExprId => event_expr_srcs,
    DeclaratorSrc => DeclId => decl_srcs,
    StmtSrc => StmtId => stmt_srcs,
    crate::hir_def::block::BlockSrc => crate::hir_def::block::LocalBlockId => stmt_srcs,
);

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum GenerateItem {
        ContAssignId(ContAssignId),
        DefParamId(DefParamId),
        GenerateBlockId(GenerateBlockId),
        DeclarationId(DeclarationId),
        StructId(StructId),
        InstantiationId(InstantiationId),
        ProcId(ProcId),
        TypedefId(TypedefId),
        SubroutineId(LocalSubroutineId),
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum GenerateBlockItem {
        ContAssignId(ContAssignId),
        DefParamId(DefParamId),
        GenerateBlockId(GenerateBlockId),
        DeclarationId(DeclarationId),
        StructId(StructId),
        InstantiationId(InstantiationId),
        ProcId(ProcId),
        TypedefId(TypedefId),
        SubroutineId(LocalSubroutineId),
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
pub enum GenerateBlockKind {
    #[default]
    Block,
    Loop {
        genvar: Option<Ident>,
        initial: ExprId,
        stop: ExprId,
        iteration: ExprId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct GenerateBlockId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct GenerateBlockLoc {
    pub cont_id: ArenaOwnerId,
    pub src: InFile<GenerateBlockSrc>,
}

pub(crate) type LowerGenerateBlockCtx<'a> = LoweringCtx<'a, GenerateBlockStore<'a>>;

impl LowerGenerateBlockCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = ArenaOwnerId::GenerateBlock(self.generate_block_id());
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
            ArenaOwnerId::GenerateBlock(self.generate_block_id()),
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

        let subroutine_id = alloc_with_source(
            self.file_id,
            &mut self.store.data.subroutines,
            &mut self.store.sources.subroutine_srcs,
            subroutine,
            func,
        );

        let subroutine_def_id = SubroutineScope::new(
            SubroutineParent::GenerateBlock(self.generate_block_id()),
            subroutine_id,
        );

        if func.end().is_some() {
            let subroutine = &mut self.store.data.subroutines[subroutine_id];
            let mut subroutine_source_map = std::mem::take(&mut subroutine.source_map);
            let mut ctx = LoweringCtx::new(
                self.db,
                self.file_id,
                subroutine_def_id.into(),
                SubroutineStore { data: subroutine, sources: &mut subroutine_source_map },
            );
            lower_subroutine_body(&mut ctx, func);
            ctx.emit_diagnostics();
            drop(ctx);
            subroutine.source_map = subroutine_source_map;
            subroutine.source_map.shrink_to_fit();
        }

        self.store.data.subroutines[subroutine_id].shrink_to_fit();

        Some(subroutine_id)
    }

    fn intern_generate_block(&self, src: GenerateBlockSrc) -> GenerateBlockId {
        self.db.intern_generate_block(GenerateBlockLoc {
            cont_id: self.generate_block_id().into(),
            src: InFile::new(self.file_id, src),
        })
    }

    fn generate_block_item_from_branch(
        &mut self,
        member: ast::Member,
    ) -> SmallVec<[GenerateBlockItem; 4]> {
        use ast::Member::*;
        match member {
            EmptyMember(_) => SmallVec::new(),
            GenerateBlock(block) => smallvec::smallvec![
                self.intern_generate_block(GenerateBlockSrc::from_generate_block(block)).into()
            ],
            LoopGenerate(loop_generate) => {
                smallvec::smallvec![self.intern_generate_block(loop_generate.into()).into()]
            }
            IfGenerate(if_generate) => self.lower_if_generate_items(if_generate),
            CaseGenerate(case_generate) => self.lower_case_generate_items(case_generate),
            member => smallvec::smallvec![self.intern_generate_block(member.into()).into()],
        }
    }

    fn lower_if_generate_items(
        &mut self,
        if_generate: ast::IfGenerate,
    ) -> SmallVec<[GenerateBlockItem; 4]> {
        self.lower_expr(if_generate.condition());

        let mut items = self.generate_block_item_from_branch(if_generate.block());
        if let Some(else_clause) = if_generate.else_clause()
            && let Some(member) = ast::Member::cast(else_clause.clause().syntax())
        {
            items.extend(self.generate_block_item_from_branch(member));
        }
        items
    }

    fn lower_case_generate_items(
        &mut self,
        case_generate: ast::CaseGenerate,
    ) -> SmallVec<[GenerateBlockItem; 4]> {
        self.lower_expr(case_generate.condition());

        let mut items = SmallVec::new();
        for item in case_generate.items().children() {
            use ast::CaseItem::*;
            match item {
                StandardCaseItem(item) => {
                    for expr in item.expressions().children() {
                        self.lower_expr(expr);
                    }
                    if let Some(member) = ast::Member::cast(item.clause().syntax()) {
                        items.extend(self.generate_block_item_from_branch(member));
                    }
                }
                DefaultCaseItem(item) => {
                    if let Some(member) = ast::Member::cast(item.clause().syntax()) {
                        items.extend(self.generate_block_item_from_branch(member));
                    }
                }
                PatternCaseItem(item) => {
                    if let Some(expr) = item.expr() {
                        self.lower_expr(expr);
                    }
                }
            }
        }
        items
    }

    fn lower_generate_member(&mut self, member: ast::Member) -> Option<GenerateBlockItem> {
        use ast::Member::*;
        let item = match member {
            ContinuousAssign(assign) => self.lower_continuous_assign(assign).into(),
            DataDeclaration(data_decl) => self.lower_data_decl(data_decl).into(),
            NetDeclaration(net_decl) => self.lower_net_decl(net_decl).into(),
            ParameterDeclarationStatement(param_decl) => {
                self.lower_param_decl_base(param_decl.parameter()).into()
            }
            TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
            GenvarDeclaration(genvar_decl) => self.lower_genvar_decl(genvar_decl).into(),
            HierarchyInstantiation(instantiation) => self.lower_instantiation(instantiation).into(),
            PrimitiveInstantiation(instantiation) => {
                self.lower_primitive_instantiation(instantiation).into()
            }
            FunctionDeclaration(fn_decl) => self.lower_subroutine_decl(fn_decl)?.into(),
            ProceduralBlock(proc) => self.lower_proc(proc).into(),
            GenerateBlock(block) => {
                self.intern_generate_block(GenerateBlockSrc::from_generate_block(block)).into()
            }
            LoopGenerate(loop_generate) => self.intern_generate_block(loop_generate.into()).into(),
            IfGenerate(if_generate) => {
                for item in self.lower_if_generate_items(if_generate) {
                    self.store.data.items.push(item.clone());
                    self.store.sources.items.push(item);
                }
                return None;
            }
            CaseGenerate(case_generate) => {
                for item in self.lower_case_generate_items(case_generate) {
                    self.store.data.items.push(item.clone());
                    self.store.sources.items.push(item);
                }
                return None;
            }
            DefParam(defparam) => self.lower_defparam(defparam).into(),
            EmptyMember(_) => return None,
            _ => return None,
        };

        Some(item)
    }

    fn lower_generate_block(&mut self, block: ast::GenerateBlock) {
        self.store.data.name =
            generate_block_name(block).and_then(|name| lower_ident_opt(Some(name)));
        self.store.data.kind = GenerateBlockKind::Block;

        for member in block.members().children() {
            let Some(item) = self.lower_generate_member(member) else {
                continue;
            };
            self.store.data.items.push(item.clone());
            self.store.sources.items.push(item);
            self.region_tree.handle_node(member.syntax());
        }

        self.region_tree.stage(block.end(), block.syntax());
        self.store.data.region_tree = self.region_tree.finish();
        self.store.sources.region_tree = self.store.data.region_tree.clone();
    }

    fn lower_loop_generate(&mut self, loop_generate: ast::LoopGenerate) {
        self.store.data.name = loop_generate
            .block()
            .as_generate_block()
            .and_then(generate_block_name)
            .and_then(|name| lower_ident_opt(Some(name)));

        let initial = self.lower_expr(loop_generate.initial_expr());
        let stop = self.lower_expr(loop_generate.stop_expr());
        let iteration = self.lower_expr(loop_generate.iteration_expr());
        self.store.data.kind = GenerateBlockKind::Loop {
            genvar: lower_ident_opt(loop_generate.identifier()),
            initial,
            stop,
            iteration,
        };

        if let Some(block) = loop_generate.block().as_generate_block() {
            for member in block.members().children() {
                let Some(item) = self.lower_generate_member(member) else {
                    continue;
                };
                self.store.data.items.push(item.clone());
                self.store.sources.items.push(item);
                self.region_tree.handle_node(member.syntax());
            }
            self.region_tree.stage(block.end(), block.syntax());
        }

        self.store.data.region_tree = self.region_tree.finish();
        self.store.sources.region_tree = self.store.data.region_tree.clone();
    }

    fn lower_single_member(&mut self, member: ast::Member) {
        if let Some(item) = self.lower_generate_member(member) {
            self.store.data.items.push(item.clone());
            self.store.sources.items.push(item);
        }

        self.store.data.region_tree = self.region_tree.finish();
        self.store.sources.region_tree = self.store.data.region_tree.clone();
    }
}

impl LowerModuleCtx<'_> {
    pub(crate) fn intern_generate_block(&self, src: GenerateBlockSrc) -> GenerateBlockId {
        self.db.intern_generate_block(GenerateBlockLoc {
            cont_id: self.module_id().into(),
            src: InFile::new(self.file_id, src),
        })
    }

    fn generate_item_from_branch(&mut self, member: ast::Member) -> SmallVec<[GenerateItem; 4]> {
        use ast::Member::*;
        match member {
            EmptyMember(_) => SmallVec::new(),
            GenerateBlock(block) => smallvec::smallvec![
                self.intern_generate_block(GenerateBlockSrc::from_generate_block(block)).into()
            ],
            LoopGenerate(loop_generate) => {
                smallvec::smallvec![self.intern_generate_block(loop_generate.into()).into()]
            }
            IfGenerate(if_generate) => self.lower_if_generate_items(if_generate),
            CaseGenerate(case_generate) => self.lower_case_generate_items(case_generate),
            member => smallvec::smallvec![self.intern_generate_block(member.into()).into()],
        }
    }

    fn lower_if_generate_items(
        &mut self,
        if_generate: ast::IfGenerate,
    ) -> SmallVec<[GenerateItem; 4]> {
        self.lower_expr(if_generate.condition());

        let mut items = self.generate_item_from_branch(if_generate.block());
        if let Some(else_clause) = if_generate.else_clause()
            && let Some(member) = ast::Member::cast(else_clause.clause().syntax())
        {
            items.extend(self.generate_item_from_branch(member));
        }
        items
    }

    fn lower_case_generate_items(
        &mut self,
        case_generate: ast::CaseGenerate,
    ) -> SmallVec<[GenerateItem; 4]> {
        self.lower_expr(case_generate.condition());

        let mut items = SmallVec::new();
        for item in case_generate.items().children() {
            use ast::CaseItem::*;
            match item {
                StandardCaseItem(item) => {
                    for expr in item.expressions().children() {
                        self.lower_expr(expr);
                    }
                    if let Some(member) = ast::Member::cast(item.clause().syntax()) {
                        items.extend(self.generate_item_from_branch(member));
                    }
                }
                DefaultCaseItem(item) => {
                    if let Some(member) = ast::Member::cast(item.clause().syntax()) {
                        items.extend(self.generate_item_from_branch(member));
                    }
                }
                PatternCaseItem(item) => {
                    if let Some(expr) = item.expr() {
                        self.lower_expr(expr);
                    }
                }
            }
        }
        items
    }

    fn lower_generate_region_member(
        &mut self,
        item: ast::Member,
        items: &mut SmallVec<[GenerateItem; 4]>,
    ) {
        use ast::Member::*;
        match item {
            ContinuousAssign(assign) => {
                items.push(self.lower_continuous_assign(assign).into());
            }
            DataDeclaration(data_decl) => {
                items.push(self.lower_data_decl(data_decl).into());
            }
            NetDeclaration(net_decl) => {
                items.push(self.lower_net_decl(net_decl).into());
            }
            EmptyMember(_) => {}
            GenvarDeclaration(genvar_decl) => {
                items.push(self.lower_genvar_decl(genvar_decl).into());
            }
            ParameterDeclarationStatement(param_decl) => {
                items.push(self.lower_param_decl_base(param_decl.parameter()).into());
            }
            TypedefDeclaration(typedef_decl) => {
                items.push(self.lower_typedef(typedef_decl).into());
            }
            HierarchyInstantiation(instantiation) => {
                items.push(self.lower_instantiation(instantiation).into());
            }
            PrimitiveInstantiation(instantiation) => {
                items.push(self.lower_primitive_instantiation(instantiation).into());
            }
            FunctionDeclaration(fn_decl) => {
                if let Some(sub_id) = self.lower_subroutine_decl(fn_decl) {
                    items.push(sub_id.into());
                }
            }
            ProceduralBlock(proc) => {
                items.push(self.lower_proc(proc).into());
            }
            GenerateBlock(block) => {
                items.push(
                    self.intern_generate_block(GenerateBlockSrc::from_generate_block(block)).into(),
                );
            }
            LoopGenerate(loop_generate) => {
                items.push(self.intern_generate_block(loop_generate.into()).into());
            }
            IfGenerate(if_generate) => {
                items.extend(self.lower_if_generate_items(if_generate));
            }
            CaseGenerate(case_generate) => {
                items.extend(self.lower_case_generate_items(case_generate));
            }
            DefParam(defparam) => {
                items.push(self.lower_defparam(defparam).into());
            }
            _ => {}
        }
    }

    pub(crate) fn lower_generate_region(
        &mut self,
        region: ast::GenerateRegion,
    ) -> GenerateRegionId {
        let mut items = SmallVec::new();

        for item in region.members().children() {
            self.lower_generate_region_member(item, &mut items);
        }

        alloc_with_source(
            self.file_id,
            &mut self.store.data.generate_regions,
            &mut self.store.sources.generate_region_srcs,
            GenerateRegion { items },
            region,
        )
    }

    pub(crate) fn lower_direct_generate_region(&mut self, item: ast::Member) -> GenerateRegionId {
        let src = GenerateRegionSrc::from_direct_member(&item);
        let mut items = SmallVec::new();
        self.lower_generate_region_member(item, &mut items);

        alloc_with_optional_source_entry(
            &mut self.store.data.generate_regions,
            &mut self.store.sources.generate_region_srcs,
            GenerateRegion { items },
            src,
        )
    }
}

pub(crate) fn generate_block_with_source_map_query(
    db: &dyn HirDb,
    generate_block_id: GenerateBlockId,
) -> (Arc<GenerateBlock>, Arc<GenerateBlockSourceMap>) {
    let GenerateBlockLoc { src: InFile { file_id, value: src }, .. } = generate_block_id.lookup(db);
    let tree = db.parse(file_id);

    let mut generate_block = GenerateBlock::default();
    let mut generate_block_source_map = GenerateBlockSourceMap::default();

    let mut lower_ctx = LoweringCtx::new(
        db,
        file_id,
        generate_block_id.into(),
        GenerateBlockStore { data: &mut generate_block, sources: &mut generate_block_source_map },
    );

    match src {
        GenerateBlockSrc::GenerateBlock { .. } => {
            if let Some(block) = src.to_node(&tree) {
                lower_ctx.lower_generate_block(block);
            }
        }
        GenerateBlockSrc::LoopGenerate { .. } => {
            if let Some(loop_generate) = src.to_node(&tree) {
                lower_ctx.lower_loop_generate(loop_generate);
            }
        }
        GenerateBlockSrc::SingleMember { .. } => {
            if let Some(member) = src.to_member(&tree) {
                lower_ctx.lower_single_member(member);
            }
        }
    }

    lower_ctx.emit_diagnostics();
    drop(lower_ctx);

    generate_block.shrink_to_fit();
    generate_block_source_map.shrink_to_fit();
    (Arc::new(generate_block), Arc::new(generate_block_source_map))
}
