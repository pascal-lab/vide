use clocking::{
    ClockingBlockDef, ClockingBlockId, ClockingBlockSrc, DefaultClockingRef, DefaultClockingRefSrc,
};
use continuous_assgin::{ContAssign, ContAssignId, ContAssignSrc};
use defparam::{DefParam, DefParamId, DefParamSrc};
use generate::{GenerateRegion, GenerateRegionId, GenerateRegionSrc};
use instantiation::{
    Instance, InstanceId, InstanceSrc, Instantiation, InstantiationId, InstantiationSrc,
    ParamAssign, ParamAssignId, ParamAssignSrc, PortConn, PortConnId, PortConnSrc,
};
use la_arena::{Arena, Idx, IdxRange};
use modport::{ModportDef, ModportId, ModportSrc};
use port::{
    NonAnsiPort, NonAnsiPortId, NonAnsiPortSrc, PortDecl, PortDeclId, PortDeclSrc, PortRef,
    PortRefId, PortRefSrc, PortSrcs, Ports,
};
use specify::{
    SpecifyBlock, SpecifyBlockId, SpecifyBlockSrc, SpecifyItem, SpecifyItemId, SpecifyItemSrc,
};
use syntax::{
    ast::{self, AstNode, PortList},
    has_name::HasName,
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
};
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use super::{
    Ident, PackageImport,
    aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
    alloc_with_source,
    block::{BlockInfo, BlockSrc, LocalBlockId},
    checker::{CheckerDef, CheckerId, CheckerSrc},
    covergroup::{
        CovergroupDef, CovergroupId, CovergroupSrc, CoverpointDef, CoverpointId, CoverpointSrc,
        CrossDef, CrossId, CrossSrc, lower_covergroup_decl, lower_coverpoint, lower_cross,
    },
    declaration::{Declaration, DeclarationId, DeclarationSrc, ParamDeclKind},
    expr::{
        Expr, ExprId, ExprSrc,
        declarator::{DeclId, Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprId, EventExprSrc},
    },
    lower::{LoweringCtx, ModuleStore, SubroutineStore},
    lower_ident_opt, lower_package_imports,
    proc::{Proc, ProcId, ProcSrc},
    stmt::{Stmt, StmtId, StmtSrc},
    subroutine::{
        LocalSubroutineId, Subroutine, SubroutineSrc, lower_subroutine, lower_subroutine_body,
    },
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::{InContainer, InFile, ScopeId},
    db::HirDb,
    file::HirFileId,
    region_tree::RegionTree,
    source_map::{
        FromSourceAst, IsNamedSrc, IsSrc, SourceAst, SourceMap, ToAstNode, ast_node_from_ptr,
        root_token_in,
    },
};

pub mod clocking;
pub mod continuous_assgin;
pub mod defparam;
pub mod generate;
pub mod instantiation;
pub mod modport;
pub mod port;
pub mod specify;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Module {
    pub name: Option<Ident>,
    pub param_ports: Option<IdxRange<Declarator>>,
    pub ports: Ports,
    pub cont_assigns: Arena<ContAssign>,
    pub defparams: Arena<DefParam>,
    pub generate_regions: Arena<GenerateRegion>,
    pub specify_blocks: Arena<SpecifyBlock>,
    pub specify_items: Arena<SpecifyItem>,
    pub declarations: Arena<Declaration>,
    pub typedefs: Arena<Typedef>,
    pub structs: Arena<StructDef>,
    pub subroutines: Arena<Subroutine>,
    pub modports: Arena<ModportDef>,
    pub default_clocking: Option<DefaultClockingRef>,
    pub clocking_blocks: Arena<ClockingBlockDef>,
    pub checkers: Arena<CheckerDef>,
    pub covergroups: Arena<CovergroupDef>,
    pub coverpoints: Arena<CoverpointDef>,
    pub crosses: Arena<CrossDef>,
    pub package_imports: Arena<PackageImport>,
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

impl Module {
    pub fn shrink_to_fit(&mut self) {
        self.ports.shrink_to_fit();
        self.cont_assigns.shrink_to_fit();
        self.defparams.shrink_to_fit();
        self.generate_regions.shrink_to_fit();
        self.specify_blocks.shrink_to_fit();
        self.specify_items.shrink_to_fit();
        self.declarations.shrink_to_fit();
        self.typedefs.shrink_to_fit();
        self.structs.shrink_to_fit();
        self.subroutines.shrink_to_fit();
        self.modports.shrink_to_fit();
        self.clocking_blocks.shrink_to_fit();
        self.checkers.shrink_to_fit();
        self.covergroups.shrink_to_fit();
        self.coverpoints.shrink_to_fit();
        self.crosses.shrink_to_fit();
        self.package_imports.shrink_to_fit();
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
pub struct ModuleSourceMap {
    pub items: Vec<ModuleItem>,
    pub region_tree: RegionTree,
    pub port_srcs: PortSrcs,
    pub assign_srcs: SourceMap<ContAssignSrc, ContAssign>,
    pub defparam_srcs: SourceMap<DefParamSrc, DefParam>,
    pub generate_region_srcs: SourceMap<GenerateRegionSrc, GenerateRegion>,
    pub specify_block_srcs: SourceMap<SpecifyBlockSrc, SpecifyBlock>,
    pub specify_item_srcs: SourceMap<SpecifyItemSrc, SpecifyItem>,
    pub declaration_srcs: SourceMap<DeclarationSrc, Declaration>,
    pub typedef_srcs: SourceMap<TypedefSrc, Typedef>,
    pub struct_srcs: SourceMap<StructSrc, StructDef>,
    pub subroutine_srcs: SourceMap<SubroutineSrc, Subroutine>,
    pub modport_srcs: SourceMap<ModportSrc, ModportDef>,
    pub default_clocking_src: Option<DefaultClockingRefSrc>,
    pub clocking_block_srcs: SourceMap<ClockingBlockSrc, ClockingBlockDef>,
    pub checker_srcs: SourceMap<CheckerSrc, CheckerDef>,
    pub covergroup_srcs: SourceMap<CovergroupSrc, CovergroupDef>,
    pub coverpoint_srcs: SourceMap<CoverpointSrc, CoverpointDef>,
    pub cross_srcs: SourceMap<CrossSrc, CrossDef>,
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

impl ModuleSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.port_srcs.shrink_to_fit();
        self.assign_srcs.shrink_to_fit();
        self.defparam_srcs.shrink_to_fit();
        self.generate_region_srcs.shrink_to_fit();
        self.specify_block_srcs.shrink_to_fit();
        self.specify_item_srcs.shrink_to_fit();
        self.declaration_srcs.shrink_to_fit();
        self.typedef_srcs.shrink_to_fit();
        self.struct_srcs.shrink_to_fit();
        self.subroutine_srcs.shrink_to_fit();
        self.modport_srcs.shrink_to_fit();
        self.clocking_block_srcs.shrink_to_fit();
        self.checker_srcs.shrink_to_fit();
        self.covergroup_srcs.shrink_to_fit();
        self.coverpoint_srcs.shrink_to_fit();
        self.cross_srcs.shrink_to_fit();
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
    Module;
    NonAnsiPortId => ports => NonAnsiPort,
    PortRefId => ports => PortRef,
    PortDeclId => ports => PortDecl,
    ContAssignId => cont_assigns => ContAssign,
    DefParamId => defparams => DefParam,
    GenerateRegionId => generate_regions => GenerateRegion,
    SpecifyBlockId => specify_blocks => SpecifyBlock,
    SpecifyItemId => specify_items => SpecifyItem,
    DeclarationId => declarations => Declaration,
    TypedefId => typedefs => Typedef,
    StructId => structs => StructDef,
    LocalSubroutineId => subroutines => Subroutine,
    ModportId => modports => ModportDef,
    ClockingBlockId => clocking_blocks => ClockingBlockDef,
    CheckerId => checkers => CheckerDef,
    CovergroupId => covergroups => CovergroupDef,
    CoverpointId => coverpoints => CoverpointDef,
    CrossId => crosses => CrossDef,
    Idx<PackageImport> => package_imports => PackageImport,
    InstantiationId => instantiations => Instantiation,
    ParamAssignId => inst_param_assigns => ParamAssign,
    InstanceId => instances => Instance,
    PortConnId => inst_port_conns => PortConn,
    ProcId => procs => Proc,
    ExprId => exprs => Expr,
    EventExprId => event_exprs => EventExpr,
    DeclId => decls => Declarator,
    StmtId => stmts => Stmt,
    LocalBlockId => stmts => BlockInfo,
);

crate::hir_def::impl_source_map_getters!(
    ModuleSourceMap;
    NonAnsiPortSrc => NonAnsiPortId => port_srcs,
    PortRefSrc => PortRefId => port_srcs,
    PortDeclSrc => PortDeclId => port_srcs,
    ContAssignSrc => ContAssignId => assign_srcs,
    DefParamSrc => DefParamId => defparam_srcs,
    GenerateRegionSrc => GenerateRegionId => generate_region_srcs,
    SpecifyBlockSrc => SpecifyBlockId => specify_block_srcs,
    SpecifyItemSrc => SpecifyItemId => specify_item_srcs,
    DeclarationSrc => DeclarationId => declaration_srcs,
    TypedefSrc => TypedefId => typedef_srcs,
    StructSrc => StructId => struct_srcs,
    SubroutineSrc => LocalSubroutineId => subroutine_srcs,
    ModportSrc => ModportId => modport_srcs,
    ClockingBlockSrc => ClockingBlockId => clocking_block_srcs,
    CheckerSrc => CheckerId => checker_srcs,
    CovergroupSrc => CovergroupId => covergroup_srcs,
    CoverpointSrc => CoverpointId => coverpoint_srcs,
    CrossSrc => CrossId => cross_srcs,
    InstantiationSrc => InstantiationId => instantiation_srcs,
    ParamAssignSrc => ParamAssignId => inst_param_assign_srcs,
    InstanceSrc => InstanceId => instance_srcs,
    PortConnSrc => PortConnId => inst_port_conn_srcs,
    ProcSrc => ProcId => proc_srcs,
    ExprSrc => ExprId => expr_srcs,
    EventExprSrc => EventExprId => event_expr_srcs,
    DeclaratorSrc => DeclId => decl_srcs,
    StmtSrc => StmtId => stmt_srcs,
    BlockSrc => LocalBlockId => stmt_srcs,
);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ModuleSrc {
    pub file_id: HirFileId,
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
    endmodule: Option<SyntaxTokenPtr>,
}

impl ModuleSrc {
    pub fn end_range(&self) -> Option<utils::text_edit::TextRange> {
        self.endmodule.map(|token| token.range())
    }
}

impl IsSrc for ModuleSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        self.node.kind()
    }

    fn range(&self) -> utils::text_edit::TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for ModuleSrc {
    fn name_kind(&self) -> Option<syntax::TokenKind> {
        self.name.map(|name| name.kind())
    }

    fn name_range(&self) -> Option<utils::text_edit::TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> ToAstNode<'a, ast::ModuleDeclaration<'a>> for ModuleSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::ModuleDeclaration<'a>> {
        ast_node_from_ptr(self.node, tree)
    }
}

impl ModuleSrc {
    pub fn from_ast(file_id: HirFileId, module: ast::ModuleDeclaration<'_>) -> Self {
        let syntax = module.syntax();
        Self {
            file_id,
            node: syntax::slang_ext::AstNodeExt::to_ptr(&module),
            name: module.name().map(|name| SyntaxTokenPtr::from_token_in(syntax, name)),
            endmodule: module.endmodule().map(|token| SyntaxTokenPtr::from_token_in(syntax, token)),
        }
    }
}

impl<'a> FromSourceAst<'a, ast::ModuleDeclaration<'a>> for ModuleSrc {
    fn from_source_ast(module: SourceAst<ast::ModuleDeclaration<'a>>) -> Self {
        let file_id = module.file_id();
        let module = module.into_inner();
        let syntax = module.syntax();
        Self {
            file_id,
            node: syntax::slang_ext::AstNodeExt::to_ptr(&module),
            name: module
                .name()
                .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token)),
            endmodule: module
                .endmodule()
                .and_then(|token| root_token_in(syntax, token).map(SyntaxTokenPtr::from_token)),
        }
    }
}

impl From<ModuleSrc> for SyntaxNodePtr {
    fn from(src: ModuleSrc) -> Self {
        src.node
    }
}

impl From<ModuleSrc> for Option<SyntaxTokenPtr> {
    fn from(src: ModuleSrc) -> Self {
        src.name
    }
}

impl Module {
    pub fn param_port_id_by_idx(&self, idx: usize) -> Option<DeclId> {
        self.param_ports.clone()?.nth(idx)
    }

    pub fn overridable_param_id_by_idx(&self, idx: usize) -> Option<DeclId> {
        self.declarations
            .values()
            .filter_map(|declaration| match declaration {
                Declaration::ParamDecl(param_decl) if param_decl.kind.is_overridable() => {
                    Some(param_decl.decls.clone())
                }
                _ => None,
            })
            .flatten()
            .nth(idx)
    }

    pub fn non_ansi_port_id_by_idx(&self, idx: usize) -> Option<NonAnsiPortId> {
        let Ports::NonAnsi { ports, .. } = &self.ports else {
            return None;
        };
        ports.iter().nth(idx).map(|(port_id, _)| port_id)
    }

    pub fn ansi_port_decl_id_by_idx(&self, idx: usize) -> Option<PortDeclId> {
        let Ports::Ansi(port_decls) = &self.ports else {
            return None;
        };
        port_decls.iter().nth(idx).map(|(port_decl_id, _)| port_decl_id)
    }
}

impl ModuleSourceMap {
    pub fn item_to_ptr(&self, item: &ModuleItem) -> Option<SyntaxNodePtr> {
        Some(match item {
            ModuleItem::ContAssignId(idx) => self.get(*idx)?.0,
            ModuleItem::DefParamId(idx) => self.get(*idx)?.0,
            ModuleItem::GenerateRegionId(idx) => self.get(*idx)?.into(),
            ModuleItem::SpecifyBlockId(idx) => self.get(*idx)?.0,
            ModuleItem::SpecifyItemId(idx) => self.get(*idx)?.into(),
            ModuleItem::DeclarationId(idx) => self.get(*idx)?.ptr(),
            ModuleItem::StructId(idx) => self.get(*idx)?.node,
            ModuleItem::InstantiationId(idx) => self.get(*idx)?.into(),
            ModuleItem::ProcId(idx) => self.get(*idx)?.0,
            ModuleItem::PortDeclId(idx) => self.get(*idx)?.ptr(),
            ModuleItem::TypedefId(idx) => self.get(*idx)?.ptr(),
            ModuleItem::SubroutineId(idx) => self.get(*idx)?.node,
            ModuleItem::ModportId(idx) => self.get(*idx)?.node,
            ModuleItem::ClockingBlockId(idx) => self.get(*idx)?.node,
            ModuleItem::CheckerId(idx) => self.get(*idx)?.node,
            ModuleItem::CovergroupId(idx) => self.get(*idx)?.node,
        })
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum ModuleItem {
        ContAssignId(ContAssignId),
        DefParamId(DefParamId),
        GenerateRegionId(GenerateRegionId),
        SpecifyBlockId(SpecifyBlockId),
        SpecifyItemId(SpecifyItemId),
        DeclarationId(DeclarationId),
        StructId(StructId),
        InstantiationId(InstantiationId),
        ProcId(ProcId),
        PortDeclId(PortDeclId),
        TypedefId(TypedefId),
        SubroutineId(LocalSubroutineId),
        ModportId(ModportId),
        ClockingBlockId(ClockingBlockId),
        CheckerId(CheckerId),
        CovergroupId(CovergroupId),
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Default)]
pub enum ModuleKind {
    #[default]
    Module,
    Interface,
    Program,
    Package,
}

impl ModuleKind {
    pub fn from_ast(decl: ast::ModuleDeclaration) -> Self {
        if decl.as_package_declaration().is_some() {
            ModuleKind::Package
        } else if decl.as_interface_declaration().is_some() {
            ModuleKind::Interface
        } else if decl.as_program_declaration().is_some() {
            ModuleKind::Program
        } else {
            ModuleKind::Module
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ModuleInfo {
    pub name: Option<Ident>,
    pub kind: ModuleKind,
}

pub type LocalModuleId = Idx<ModuleInfo>;
pub type ModuleId = InFile<LocalModuleId>;
pub type PackageId = ModuleId;

pub(crate) type LowerModuleCtx<'a> = LoweringCtx<'a, ModuleStore<'a>>;

impl LowerModuleCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = ScopeId::Module(self.module_id());
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
            ScopeId::Module(self.module_id()),
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

        let subroutine_def_id = InContainer::new(self.module_id().into(), subroutine_id);

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

    pub(crate) fn lower_module_decl(&mut self, decl: ast::ModuleDeclaration) {
        let header = decl.header();
        let has_param_ports = header.parameters().is_some();
        if let Some(param_ports) = header.parameters() {
            let mut inherited_kind = ParamDeclKind::Parameter;
            for decls in param_ports.declarations().children() {
                let decl_id = self.lower_param_decl_base_with_context(
                    decls,
                    Some(inherited_kind),
                    false,
                    true,
                );
                if let Declaration::ParamDecl(param_decl) = self.store.data.get(decl_id) {
                    inherited_kind = param_decl.kind;
                }
                self.region_tree.handle_node(decls.syntax());
            }

            let mut decls = self.store.data.decls.iter().map(|(id, _)| id);
            if let Some(first) = decls.next() {
                let last = decls.next_back().unwrap_or(first);
                self.store.data.param_ports = Some(IdxRange::new_inclusive(first..=last));
            }

            self.region_tree.stage(param_ports.close_paren(), param_ports.syntax());
        }

        match header.ports() {
            Some(PortList::AnsiPortList(port_list)) => self.lower_ansi_ports(port_list),
            Some(PortList::NonAnsiPortList(port_list)) => self.lower_nonansi_port(port_list),
            Some(PortList::WildcardPortList(port_list)) => self.lower_wildcard_ports(port_list),
            None => {}
        };

        for member in decl.members().children() {
            use ast::Member::*;
            let idx = match member {
                // Assignments
                ContinuousAssign(assign) => self.lower_continuous_assign(assign).into(),

                // Declarations
                DataDeclaration(data_decl) => self.lower_data_decl(data_decl).into(),
                NetDeclaration(net_decl) => self.lower_net_decl(net_decl).into(),
                LocalVariableDeclaration(_) => continue,
                ParameterDeclarationStatement(param_decl) => self
                    .lower_param_decl_base_with_context(
                        param_decl.parameter(),
                        None,
                        has_param_ports,
                        false,
                    )
                    .into(),
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                GenvarDeclaration(genvar_decl) => self.lower_genvar_decl(genvar_decl).into(),
                NetTypeDeclaration(_)
                | ForwardTypedefDeclaration(_)
                | UserDefinedNetDeclaration(_) => {
                    continue;
                }

                // Instantiations
                HierarchyInstantiation(instantiation) => {
                    self.lower_instantiation(instantiation).into()
                }
                PrimitiveInstantiation(instantiation) => {
                    self.lower_primitive_instantiation(instantiation).into()
                }
                CheckerInstantiation(instantiation) => {
                    self.lower_checker_instantiation(instantiation).into()
                }

                // Subroutines
                FunctionDeclaration(fn_decl) => match self.lower_subroutine_decl(fn_decl) {
                    Some(sub_id) => sub_id.into(),
                    None => continue,
                },

                // Procedural blocks
                ProceduralBlock(proc) => self.lower_proc(proc).into(),

                // Ports
                PortDeclaration(port) => self.lower_port_decl(port).into(),
                ExplicitAnsiPort(_) | ImplicitAnsiPort(_) => continue,

                // Imports
                PackageImportDeclaration(import_decl) => {
                    for import in lower_package_imports(import_decl) {
                        self.store.data.package_imports.alloc(import);
                    }
                    continue;
                }

                // Aggregates
                ClassDeclaration(_) => continue,

                // Nested modules/interfaces/programs
                ModuleDeclaration(_) => continue,

                // Generate constructs
                GenerateRegion(region) => self.lower_generate_region(region).into(),
                gen_item @ GenerateBlock(_)
                | gen_item @ IfGenerate(_)
                | gen_item @ CaseGenerate(_)
                | gen_item @ LoopGenerate(_) => self.lower_direct_generate_region(gen_item).into(),

                // Timing and clocking
                TimeUnitsDeclaration(_) | ClockingItem(_) => continue,
                DefaultClockingReference(reference) => {
                    self.lower_default_clocking_reference(reference);
                    self.region_tree.handle_node(member.syntax());
                    continue;
                }
                ClockingDeclaration(clocking) => self.lower_clocking_declaration(clocking).into(),

                // Assertions and properties
                PropertyDeclaration(_)
                | SequenceDeclaration(_)
                | ImmediateAssertionMember(_)
                | ConcurrentAssertionMember(_) => continue,

                // Coverage
                CovergroupDeclaration(covergroup) => self.lower_covergroup_decl(covergroup).into(),
                Coverpoint(_) | CoverCross(_) | CoverageBins(_) | BinsSelection(_)
                | CoverageOption(_) => continue,

                // Specify blocks
                SpecifyBlock(block) => self.lower_specify_block(block).into(),
                PathDeclaration(path) => self.lower_specify_path_item(path).into(),
                ConditionalPathDeclaration(path) => {
                    self.lower_conditional_specify_path_item(path).into()
                }
                IfNonePathDeclaration(path) => self.lower_ifnone_specify_path_item(path).into(),
                SystemTimingCheck(timing) => self.lower_system_timing_check_item(timing).into(),
                PulseStyleDeclaration(pulse) => self.lower_pulse_style_item(pulse).into(),
                DefaultSkewItem(_) => continue,
                SpecparamDeclaration(specparam_decl) => {
                    self.lower_specparam_decl(specparam_decl).into()
                }

                // DPI and external
                DPIImport(_)
                | DPIExport(_)
                | ExternInterfaceMethod(_)
                | ExternModuleDecl(_)
                | ExternUdpDecl(_) => continue,

                // UDP
                UdpDeclaration(_) => continue,

                // Defparam
                DefParam(defparam) => self.lower_defparam(defparam).into(),

                // Net alias
                NetAlias(_) => continue,

                // Modport
                ModportDeclaration(modport) => {
                    for modport_id in self.lower_modport_declaration(modport) {
                        self.store.sources.items.push(modport_id.into());
                    }
                    self.region_tree.handle_node(member.syntax());
                    continue;
                }
                ModportClockingPort(_)
                | ModportSimplePortList(_)
                | ModportSubroutinePortList(_) => continue,

                // Class members (shouldn't appear in module but handle anyway)
                ClassPropertyDeclaration(_)
                | ClassMethodDeclaration(_)
                | ClassMethodPrototype(_) => continue,

                // Checker
                CheckerDeclaration(checker_decl) => self.lower_checker_decl(checker_decl).into(),
                CheckerDataDeclaration(_) => continue,

                // Constraints
                ConstraintDeclaration(_) | ConstraintPrototype(_) => continue,

                // Config
                ConfigDeclaration(_) => continue,

                // Bind
                BindDirective(_) => continue,

                // Package exports
                PackageExportDeclaration(_) | PackageExportAllDeclaration(_) => continue,

                // Library
                LibraryDeclaration(_) | LibraryIncludeStatement(_) => continue,

                // Let declaration
                LetDeclaration(_) => continue,

                // Default disable
                DefaultDisableDeclaration(_) => continue,

                // Elaboration system task
                ElabSystemTask(_) => continue,

                // Anonymous program
                AnonymousProgram(_) => continue,

                // Empty member - skip
                EmptyMember(_) => continue,
            };
            self.store.sources.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }
        self.region_tree.stage(decl.endmodule(), decl.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }
}

pub(crate) fn module_with_source_map_query(
    db: &dyn HirDb,
    module_id @ InFile { value: local_module_id, file_id }: ModuleId,
) -> (Arc<Module>, Arc<ModuleSourceMap>) {
    let (file, file_source_map) = db.hir_file_with_source_map(file_id);
    let tree = db.parse(file_id);

    let module_info = file.get(local_module_id);
    let mut module = Module { name: module_info.name.clone(), ..Default::default() };
    let mut module_source_map = ModuleSourceMap::default();

    let Some(ast_module) = file_source_map.get(local_module_id).and_then(|src| src.to_node(&tree))
    else {
        return (Arc::new(module), Arc::new(module_source_map));
    };

    let mut lower_ctx = LoweringCtx::new(
        db,
        file_id,
        module_id.into(),
        ModuleStore { data: &mut module, sources: &mut module_source_map },
    );
    lower_ctx.lower_module_decl(ast_module);
    lower_ctx.emit_diagnostics();

    module.shrink_to_fit();
    module_source_map.shrink_to_fit();
    (Arc::new(module), Arc::new(module_source_map))
}
