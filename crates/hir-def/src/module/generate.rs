use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxToken,
    ast::{self, AstNode},
};
use triomphe::Arc;

use super::LowerModuleCtx;
use crate::{
    Ident,
    aggregate::{StructId, lower_struct_def},
    alloc_with_source,
    ast_id_map::SourceAstId,
    body::{Body, BodyItem, BodySourceMap},
    container::{ArenaOwnerId, InFile},
    db::HirDefDb,
    expr::ExprId,
    lower::{GenerateBlockStore, LoweringCtx},
    lower_ident_opt,
    owner::{OwnerId, OwnerKind},
    source_map::Lowered,
    subroutine::{LocalSubroutineId, lower_subroutine},
    typedef::{Typedef, TypedefId, lower_typedef_data_ty},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GenerateRegion {
    pub items: SmallVec<[GenerateItem; 4]>,
}

pub type GenerateRegionId = Idx<GenerateRegion>;

pub type GenerateRegionSrc = SourceAstId;
pub type GenerateBlockSrc = SourceAstId;

/// Canonical syntax node for a generate owner. A loop-generate owns the loop
/// node, not its nested begin/end block.
pub(crate) fn generate_block_source_node(block: ast::GenerateBlock<'_>) -> syntax::SyntaxNode<'_> {
    block
        .syntax()
        .parent()
        .filter(|parent| ast::LoopGenerate::can_cast(parent.kind()))
        .unwrap_or_else(|| block.syntax())
}

pub(crate) fn generate_block_name(block: ast::GenerateBlock<'_>) -> Option<SyntaxToken<'_>> {
    block
        .label()
        .and_then(|label| label.name())
        .or_else(|| block.begin_name().and_then(|name| name.name()))
}

pub type GenerateBlock = Body;
pub type GenerateBlockSourceMap = BodySourceMap;
pub type GenerateItem = BodyItem;
pub type GenerateBlockItem = BodyItem;

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenerateBlockId(Arc<GenerateBlockLoc>);

impl GenerateBlockId {
    pub fn new(loc: GenerateBlockLoc) -> Self {
        Self(Arc::new(loc))
    }

    pub fn loc(&self) -> &GenerateBlockLoc {
        &self.0
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct GenerateBlockLoc {
    pub cont_id: ArenaOwnerId,
    pub src: InFile<GenerateBlockSrc>,
}

pub(crate) type LowerGenerateBlockCtx<'a> = LoweringCtx<GenerateBlockStore<'a>>;

impl LowerGenerateBlockCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = self.current_arena_owner();
        let struct_def = lower_struct_def(struct_ty, container_id, |ty| self.lower_data_ty(ty));

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.structs,
            &mut self.store.sources.struct_srcs,
            struct_def,
            struct_ty,
        )
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());

        let typedef_id = alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.typedefs,
            &mut self.store.sources.typedef_srcs,
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

        self.store.data.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_subroutine_decl(
        &mut self,
        func: ast::FunctionDeclaration,
    ) -> Option<LocalSubroutineId> {
        // Only the skeleton is lowered here; the body is lowered on first
        // access by subroutine_body_with_source_map.
        let subroutine = lower_subroutine(&func, |ty| self.lower_data_ty(ty))?;

        let subroutine_id = alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.subroutines,
            &mut self.store.sources.subroutine_srcs,
            subroutine,
            func,
        );

        Some(subroutine_id)
    }

    fn intern_generate_node(&self, node: syntax::SyntaxNode<'_>) -> GenerateBlockId {
        GenerateBlockId::new(GenerateBlockLoc {
            cont_id: self.current_arena_owner(),
            src: InFile::new(self.file_id, self.source_id(node)),
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
                self.intern_generate_node(generate_block_source_node(block)).into()
            ],
            LoopGenerate(loop_generate) => {
                smallvec::smallvec![self.intern_generate_node(loop_generate.syntax()).into()]
            }
            IfGenerate(if_generate) => self.lower_if_generate_items(if_generate),
            CaseGenerate(case_generate) => self.lower_case_generate_items(case_generate),
            member => smallvec::smallvec![self.intern_generate_node(member.syntax()).into()],
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
                self.intern_generate_node(generate_block_source_node(block)).into()
            }
            LoopGenerate(loop_generate) => self.intern_generate_node(loop_generate.syntax()).into(),
            IfGenerate(if_generate) => {
                for item in self.lower_if_generate_items(if_generate) {
                    self.store.data.items.push(item);
                }
                return None;
            }
            CaseGenerate(case_generate) => {
                for item in self.lower_case_generate_items(case_generate) {
                    self.store.data.items.push(item.clone());
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
        self.store.data.generate_kind = GenerateBlockKind::Block;

        for member in block.members().children() {
            let Some(item) = self.lower_generate_member(member) else {
                continue;
            };
            self.store.data.items.push(item.clone());
            self.region_tree.handle_node(member.syntax());
        }

        self.store.sources.region_tree = self.region_tree.finish();
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
        self.store.data.generate_kind = GenerateBlockKind::Loop {
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
                self.region_tree.handle_node(member.syntax());
            }
            self.region_tree.stage(block.end(), block.syntax());
        }

        self.store.sources.region_tree = self.region_tree.finish();
    }

    fn lower_single_member(&mut self, member: ast::Member) {
        if let Some(item) = self.lower_generate_member(member) {
            self.store.data.items.push(item.clone());
        }

        self.store.sources.region_tree = self.region_tree.finish();
    }
}

impl LowerModuleCtx<'_> {
    pub(crate) fn intern_generate_node(&self, node: syntax::SyntaxNode<'_>) -> GenerateBlockId {
        GenerateBlockId::new(GenerateBlockLoc {
            cont_id: self.current_arena_owner(),
            src: InFile::new(self.file_id, self.source_id(node)),
        })
    }

    fn generate_item_from_branch(&mut self, member: ast::Member) -> SmallVec<[GenerateItem; 4]> {
        use ast::Member::*;
        match member {
            EmptyMember(_) => SmallVec::new(),
            GenerateBlock(block) => smallvec::smallvec![
                self.intern_generate_node(generate_block_source_node(block)).into()
            ],
            LoopGenerate(loop_generate) => {
                smallvec::smallvec![self.intern_generate_node(loop_generate.syntax()).into()]
            }
            IfGenerate(if_generate) => self.lower_if_generate_items(if_generate),
            CaseGenerate(case_generate) => self.lower_case_generate_items(case_generate),
            member => smallvec::smallvec![self.intern_generate_node(member.syntax()).into()],
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
                items.push(self.intern_generate_node(generate_block_source_node(block)).into());
            }
            LoopGenerate(loop_generate) => {
                items.push(self.intern_generate_node(loop_generate.syntax()).into());
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
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.generate_regions,
            &mut self.store.sources.generate_region_srcs,
            GenerateRegion { items },
            region,
        )
    }

    pub(crate) fn lower_direct_generate_region(&mut self, item: ast::Member) -> GenerateRegionId {
        let source = self.source_id(item.syntax());
        let mut items = SmallVec::new();
        self.lower_generate_region_member(item, &mut items);
        crate::alloc_with_source_entry(
            &mut self.store.data.generate_regions,
            &mut self.store.sources.generate_region_srcs,
            GenerateRegion { items },
            source,
        )
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn generate_block_with_source_map(
    db: &dyn HirDefDb,
    owner: OwnerId,
) -> Arc<Lowered<GenerateBlock>> {
    debug_assert_eq!(owner.kind(db), OwnerKind::GenerateBlock);
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(node) = db.ast_id_map(file_id).node(owner.ast_id(db), &tree) else {
        return Arc::new(Lowered::new(file_id, Body::default(), BodySourceMap::default()));
    };

    enum SourceKind {
        GenerateBlock,
        LoopGenerate,
        SingleMember,
    }
    let source_kind = if ast::LoopGenerate::can_cast(node.kind()) {
        SourceKind::LoopGenerate
    } else if ast::GenerateBlock::can_cast(node.kind()) {
        SourceKind::GenerateBlock
    } else if ast::Member::can_cast(node.kind()) {
        SourceKind::SingleMember
    } else {
        unreachable!("generate owner must point to generate syntax")
    };

    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let mut lower_ctx = LoweringCtx::new(
        db,
        owner,
        GenerateBlockStore { data: &mut body, sources: &mut source_map },
    );

    match source_kind {
        SourceKind::GenerateBlock => lower_ctx.lower_generate_block(
            ast::GenerateBlock::cast(node).expect("generate owner kind was checked"),
        ),
        SourceKind::LoopGenerate => lower_ctx.lower_loop_generate(
            ast::LoopGenerate::cast(node).expect("loop-generate owner kind was checked"),
        ),
        SourceKind::SingleMember => lower_ctx.lower_single_member(
            ast::Member::cast(node).expect("single-member owner kind was checked"),
        ),
    }

    let diagnostics = lower_ctx.emit_diagnostics();
    drop(lower_ctx);
    source_map.diagnostics = diagnostics;
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new(file_id, body, source_map))
}

pub(crate) fn set_generate_block_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    generate_block_with_source_map::set_lru_capacity(db, capacity);
}
