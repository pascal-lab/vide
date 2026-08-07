use la_arena::{Arena, Idx};
use smallvec::SmallVec;
use syntax::{
    SyntaxToken,
    ast::{self, AstNode},
};
use triomphe::Arc;
use utils::define_enum_deriving_from;

use super::{
    LowerModuleCtx,
    continuous_assign::{ContAssign, ContAssignId, ContAssignSrc},
    defparam::{DefParam, DefParamId, DefParamSrc},
    instantiation::{
        Instance, InstanceId, InstanceSrc, Instantiation, InstantiationId, InstantiationSrc,
        ParamAssign, ParamAssignId, ParamAssignSrc, PortConn, PortConnId, PortConnSrc,
    },
};
use crate::{
    Ident,
    aggregate::{StructId, lower_struct_def},
    alloc_with_source,
    ast_id_map::SourceAstId,
    body::{Body, BodySourceMap, OwnerLowering},
    container::{ArenaOwnerId, InFile},
    db::HirDefDb,
    declaration::DeclarationId,
    expr::ExprId,
    lower::{GenerateBlockStore, LoweringCtx},
    lower_ident_opt,
    owner::{OwnerId, OwnerKind},
    proc::{Proc, ProcId},
    region_tree::RegionTree,
    source_map::{DiagnosticSource, Lowered, LoweredData, LoweringDiagnostic, SourceMap},
    subroutine::{LocalSubroutineId, Subroutine, lower_subroutine},
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

#[derive(Default, Debug, PartialEq, Eq)]
pub struct GenerateBlock {
    pub name: Option<Ident>,
    pub kind: GenerateBlockKind,
    pub items: Vec<GenerateBlockItem>,
    pub cont_assigns: Arena<ContAssign>,
    pub defparams: Arena<DefParam>,
    pub subroutines: Arena<Subroutine>,
    pub instantiations: Arena<Instantiation>,
    pub inst_param_assigns: Arena<ParamAssign>,
    pub instances: Arena<Instance>,
    pub inst_port_conns: Arena<PortConn>,
    pub procs: Arena<Proc>,
}
impl GenerateBlock {
    pub fn shrink_to_fit(&mut self) {
        self.cont_assigns.shrink_to_fit();
        self.defparams.shrink_to_fit();
        self.subroutines.shrink_to_fit();
        self.instantiations.shrink_to_fit();
        self.inst_param_assigns.shrink_to_fit();
        self.instances.shrink_to_fit();
        self.inst_port_conns.shrink_to_fit();
        self.procs.shrink_to_fit();
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct GenerateBlockSourceMap {
    pub region_tree: RegionTree,
    pub assign_srcs: SourceMap<ContAssign>,
    pub defparam_srcs: SourceMap<DefParam>,
    pub subroutine_srcs: SourceMap<Subroutine>,
    pub instantiation_srcs: SourceMap<Instantiation>,
    pub inst_param_assign_srcs: SourceMap<ParamAssign>,
    pub instance_srcs: SourceMap<Instance>,
    pub inst_port_conn_srcs: SourceMap<PortConn>,
    pub proc_srcs: SourceMap<Proc>,
    pub diagnostics: Vec<LoweringDiagnostic>,
}
impl LoweredData for GenerateBlock {
    type SourceMap = GenerateBlockSourceMap;
}

impl DiagnosticSource for GenerateBlockSourceMap {
    fn diagnostics(&self) -> &[LoweringDiagnostic] {
        &self.diagnostics
    }
}

impl GenerateBlockSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.assign_srcs.shrink_to_fit();
        self.defparam_srcs.shrink_to_fit();
        self.subroutine_srcs.shrink_to_fit();
        self.instantiation_srcs.shrink_to_fit();
        self.inst_param_assign_srcs.shrink_to_fit();
        self.instance_srcs.shrink_to_fit();
        self.inst_port_conn_srcs.shrink_to_fit();
        self.proc_srcs.shrink_to_fit();
        self.diagnostics.shrink_to_fit();
    }
}

crate::impl_arena_getters!(
    GenerateBlock;
    ContAssignId => cont_assigns => ContAssign,
    DefParamId => defparams => DefParam,
    LocalSubroutineId => subroutines => Subroutine,
    InstantiationId => instantiations => Instantiation,
    ParamAssignId => inst_param_assigns => ParamAssign,
    InstanceId => instances => Instance,
    PortConnId => inst_port_conns => PortConn,
    ProcId => procs => Proc,
);

crate::impl_source_map_getters!(
    GenerateBlockSourceMap;
    ContAssignId => assign_srcs,
    DefParamId => defparam_srcs,
    LocalSubroutineId => subroutine_srcs,
    InstantiationId => instantiation_srcs,
    ParamAssignId => inst_param_assign_srcs,
    InstanceId => instance_srcs,
    PortConnId => inst_port_conn_srcs,
    ProcId => proc_srcs,
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
        self.store.data.kind = GenerateBlockKind::Block;

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
fn generate_block_lowering(db: &dyn HirDefDb, owner: OwnerId) -> Arc<OwnerLowering<GenerateBlock>> {
    debug_assert_eq!(owner.kind(db), OwnerKind::GenerateBlock);
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(node) = db.ast_id_map(file_id).node(owner.ast_id(db), &tree) else {
        return Arc::new(OwnerLowering::new(
            file_id,
            GenerateBlock::default(),
            GenerateBlockSourceMap::default(),
            Body::default(),
            BodySourceMap::default(),
        ));
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

    let mut generate_block = GenerateBlock::default();
    let mut generate_block_source_map = GenerateBlockSourceMap::default();
    let mut body = Body::default();
    let mut body_source_map = BodySourceMap::default();
    let mut lower_ctx = LoweringCtx::new(
        db,
        owner,
        GenerateBlockStore {
            data: &mut generate_block,
            sources: &mut generate_block_source_map,
            body: &mut body,
            body_sources: &mut body_source_map,
        },
    );

    match source_kind {
        SourceKind::GenerateBlock => {
            lower_ctx.lower_generate_block(
                ast::GenerateBlock::cast(node).expect("generate owner kind was checked"),
            );
        }
        SourceKind::LoopGenerate => {
            lower_ctx.lower_loop_generate(
                ast::LoopGenerate::cast(node).expect("loop-generate owner kind was checked"),
            );
        }
        SourceKind::SingleMember => {
            lower_ctx.lower_single_member(
                ast::Member::cast(node).expect("single-member owner kind was checked"),
            );
        }
    }

    let diagnostics = lower_ctx.emit_diagnostics();
    drop(lower_ctx);
    generate_block_source_map.diagnostics = diagnostics.clone();
    body_source_map.diagnostics = diagnostics;
    generate_block.shrink_to_fit();
    generate_block_source_map.shrink_to_fit();
    body.shrink_to_fit();
    body_source_map.shrink_to_fit();
    Arc::new(OwnerLowering::new(
        file_id,
        generate_block,
        generate_block_source_map,
        body,
        body_source_map,
    ))
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn generate_block_with_source_map(
    db: &dyn HirDefDb,
    owner: OwnerId,
) -> Arc<Lowered<GenerateBlock>> {
    generate_block_lowering(db, owner).structure.clone()
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn generate_block_body_with_source_map(
    db: &dyn HirDefDb,
    owner: OwnerId,
) -> Arc<Lowered<Body>> {
    generate_block_lowering(db, owner).body.clone()
}

pub(crate) fn set_generate_block_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    generate_block_lowering::set_lru_capacity(db, capacity);
    generate_block_with_source_map::set_lru_capacity(db, capacity);
    generate_block_body_with_source_map::set_lru_capacity(db, capacity);
}
