use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxToken,
    ast::{self, AstNode},
};
use triomphe::Arc;

use super::LowerModuleCtx;
use crate::{
    Ident, alloc_with_source,
    body::{Body, BodyItem, BodySourceMap},
    db::HirDefDb,
    expr::ExprId,
    lower::{BodyStore, LoweringCtx, LoweringSyntax},
    lower_ident_opt, lower_package_imports,
    owner::{OwnerId, OwnerKind},
    source_map::Lowered,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GenerateRegion {
    pub items: SmallVec<[BodyItem; 4]>,
}

pub type GenerateRegionId = Idx<GenerateRegion>;

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
pub(crate) type LowerGenerateBlockCtx<'a> = LoweringCtx<BodyStore<'a>>;

impl LowerGenerateBlockCtx<'_> {
    fn generate_block_item_from_branch(&mut self, member: ast::Member) -> SmallVec<[BodyItem; 4]> {
        use ast::Member::*;
        match member {
            EmptyMember(_) => SmallVec::new(),
            GenerateBlock(block) => smallvec::smallvec![BodyItem::GenerateBlockOwner(
                self.intern_generate_node(generate_block_source_node(block)),
            )],
            LoopGenerate(loop_generate) => {
                smallvec::smallvec![BodyItem::GenerateBlockOwner(
                    self.intern_generate_node(loop_generate.syntax())
                )]
            }
            IfGenerate(if_generate) => self.lower_if_generate_items_block(if_generate),
            CaseGenerate(case_generate) => self.lower_case_generate_items_block(case_generate),
            member => smallvec::smallvec![BodyItem::GenerateBlockOwner(
                self.intern_generate_node(member.syntax())
            )],
        }
    }

    fn lower_if_generate_items_block(
        &mut self,
        if_generate: ast::IfGenerate,
    ) -> SmallVec<[BodyItem; 4]> {
        self.lower_expr(if_generate.condition());

        let mut items = self.generate_block_item_from_branch(if_generate.block());
        if let Some(else_clause) = if_generate.else_clause()
            && let Some(member) = ast::Member::cast(else_clause.clause().syntax())
        {
            items.extend(self.generate_block_item_from_branch(member));
        }
        items
    }

    fn lower_case_generate_items_block(
        &mut self,
        case_generate: ast::CaseGenerate,
    ) -> SmallVec<[BodyItem; 4]> {
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

    fn lower_generate_member(&mut self, member: ast::Member) -> Option<BodyItem> {
        use ast::Member::*;
        let item = match member {
            ContinuousAssign(assign) => self.lower_continuous_assign(assign).into(),
            DataDeclaration(data_decl) => self.lower_data_decl(data_decl).into(),
            NetDeclaration(net_decl) => self.lower_net_decl(net_decl).into(),
            UserDefinedNetDeclaration(net_decl) => {
                BodyItem::DeclarationId(self.lower_user_defined_net_decl(net_decl)?)
            }
            ParameterDeclarationStatement(param_decl) => {
                self.lower_param_decl_base(param_decl.parameter()).into()
            }
            TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
            GenvarDeclaration(genvar_decl) => self.lower_genvar_decl(genvar_decl).into(),
            HierarchyInstantiation(instantiation) => self.lower_instantiation(instantiation).into(),
            PrimitiveInstantiation(instantiation) => {
                self.lower_primitive_instantiation(instantiation).into()
            }
            FunctionDeclaration(fn_decl) => {
                BodyItem::SubroutineOwner(self.lower_subroutine_decl(fn_decl)?)
            }
            BindDirective(directive) => {
                BodyItem::BindDirectiveId(self.lower_bind_directive(directive)?)
            }
            DPIImport(declaration) => BodyItem::DpiImportId(self.lower_dpi_import(declaration)?),
            DPIExport(declaration) => BodyItem::DpiExportId(self.lower_dpi_export(declaration)?),
            ForwardTypedefDeclaration(declaration) => {
                BodyItem::TypedefId(self.lower_forward_typedef(declaration)?)
            }
            NetTypeDeclaration(declaration) => {
                BodyItem::NetTypeDeclId(self.lower_net_type_decl(declaration)?)
            }
            NetAlias(alias) => BodyItem::NetAliasId(self.lower_net_alias(alias)?),
            ProceduralBlock(proc) => self.lower_proc(proc).into(),
            GenerateBlock(block) => BodyItem::GenerateBlockOwner(
                self.intern_generate_node(generate_block_source_node(block)),
            ),
            LoopGenerate(loop_generate) => {
                BodyItem::GenerateBlockOwner(self.intern_generate_node(loop_generate.syntax()))
            }
            IfGenerate(if_generate) => {
                for item in self.lower_if_generate_items_block(if_generate) {
                    self.store.data.items.push(item);
                }
                return None;
            }
            CaseGenerate(case_generate) => {
                for item in self.lower_case_generate_items_block(case_generate) {
                    self.store.data.items.push(item.clone());
                }
                return None;
            }
            DefParam(defparam) => self.lower_defparam(defparam).into(),
            PackageImportDeclaration(import_decl) => {
                for import in
                    lower_package_imports(import_decl, self.source_id(import_decl.syntax()))
                {
                    self.store.data.package_imports.alloc(import);
                }
                return None;
            }
            EmptyMember(_) => return None,
            unsupported => {
                self.report_unsupported(unsupported.syntax(), "unsupported generate member");
                return None;
            }
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
        }
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
            }
        }
    }

    fn lower_single_member(&mut self, member: ast::Member) {
        if let Some(item) = self.lower_generate_member(member) {
            self.store.data.items.push(item.clone());
        }
    }
}

impl LowerModuleCtx<'_> {
    pub(crate) fn intern_generate_node(&self, node: syntax::SyntaxNode<'_>) -> OwnerId {
        self.owner_for_node(node, OwnerKind::GenerateBlock)
            .expect("every lowered generate node must have a canonical owner")
    }

    fn generate_item_from_branch(&mut self, member: ast::Member) -> SmallVec<[BodyItem; 4]> {
        use ast::Member::*;
        match member {
            EmptyMember(_) => SmallVec::new(),
            GenerateBlock(block) => smallvec::smallvec![BodyItem::GenerateBlockOwner(
                self.intern_generate_node(generate_block_source_node(block)),
            )],
            LoopGenerate(loop_generate) => {
                smallvec::smallvec![BodyItem::GenerateBlockOwner(
                    self.intern_generate_node(loop_generate.syntax())
                )]
            }
            IfGenerate(if_generate) => self.lower_if_generate_items(if_generate),
            CaseGenerate(case_generate) => self.lower_case_generate_items(case_generate),
            member => smallvec::smallvec![BodyItem::GenerateBlockOwner(
                self.intern_generate_node(member.syntax())
            )],
        }
    }

    fn lower_if_generate_items(&mut self, if_generate: ast::IfGenerate) -> SmallVec<[BodyItem; 4]> {
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
    ) -> SmallVec<[BodyItem; 4]> {
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
        items: &mut SmallVec<[BodyItem; 4]>,
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
            UserDefinedNetDeclaration(net_decl) => {
                if let Some(declaration) = self.lower_user_defined_net_decl(net_decl) {
                    items.push(declaration.into());
                }
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
            ForwardTypedefDeclaration(typedef_decl) => {
                if let Some(typedef) = self.lower_forward_typedef(typedef_decl) {
                    items.push(typedef.into());
                }
            }
            NetTypeDeclaration(declaration) => {
                if let Some(net_type) = self.lower_net_type_decl(declaration) {
                    items.push(net_type.into());
                }
            }
            NetAlias(alias) => {
                if let Some(net_alias) = self.lower_net_alias(alias) {
                    items.push(net_alias.into());
                }
            }
            HierarchyInstantiation(instantiation) => {
                items.push(self.lower_instantiation(instantiation).into());
            }
            PrimitiveInstantiation(instantiation) => {
                items.push(self.lower_primitive_instantiation(instantiation).into());
            }
            FunctionDeclaration(fn_decl) => {
                if let Some(owner) = self.lower_subroutine_decl(fn_decl) {
                    items.push(BodyItem::SubroutineOwner(owner));
                }
            }
            BindDirective(directive) => {
                if let Some(bind) = self.lower_bind_directive(directive) {
                    items.push(bind.into());
                }
            }
            ProceduralBlock(proc) => {
                items.push(self.lower_proc(proc).into());
            }
            GenerateBlock(block) => {
                items.push(BodyItem::GenerateBlockOwner(
                    self.intern_generate_node(generate_block_source_node(block)),
                ));
            }
            LoopGenerate(loop_generate) => {
                items.push(BodyItem::GenerateBlockOwner(
                    self.intern_generate_node(loop_generate.syntax()),
                ));
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
            PackageImportDeclaration(import_decl) => {
                for import in
                    lower_package_imports(import_decl, self.source_id(import_decl.syntax()))
                {
                    self.store.data.package_imports.alloc(import);
                }
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

pub(crate) fn lower_generate_owner(
    db: &dyn HirDefDb,
    owner: OwnerId,
    syntax: &LoweringSyntax,
) -> Arc<Lowered<Body>> {
    debug_assert_eq!(owner.kind(db), OwnerKind::GenerateBlock);
    let file_id = syntax.file_id;
    let tree = syntax.tree.clone();
    let Some(node) = syntax.ast_ids.node(owner.ast_id(db), &tree) else {
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
    let mut lower_ctx = LoweringCtx::new_with_syntax(
        db,
        owner,
        syntax,
        BodyStore { data: &mut body, sources: &mut source_map },
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
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new_with_diagnostics(file_id, body, source_map, diagnostics))
}
