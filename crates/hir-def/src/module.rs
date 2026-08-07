use clocking::{
    ClockingBlockDef, ClockingBlockId, ClockingBlockSrc, DefaultClockingRef, DefaultClockingRefSrc,
};
use continuous_assign::{ContAssign, ContAssignId, ContAssignSrc};
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
use preproc_expand::file::HirFileId;
use specify::{
    SpecifyBlock, SpecifyBlockId, SpecifyBlockSrc, SpecifyItem, SpecifyItemId, SpecifyItemSrc,
};
use syntax::{
    ast::{self, AstNode, PortList},
    has_name::HasName,
};
use triomphe::Arc;
use utils::{define_enum_deriving_from, get::Get};

use super::{
    Ident, PackageImport,
    aggregate::{StructId, lower_struct_def},
    alloc_with_source,
    checker::{CheckerDef, CheckerId, CheckerSrc},
    covergroup::{
        CovergroupDef, CovergroupId, CovergroupSrc, CoverpointDef, CoverpointId, CoverpointSrc,
        CrossDef, CrossId, CrossSrc, lower_covergroup_decl, lower_coverpoint, lower_cross,
    },
    declaration::{Declaration, DeclarationId, ParamDeclKind},
    expr::declarator::{DeclId, Declarator},
    lower::{LoweringCtx, ModuleStore},
    lower_ident_opt, lower_package_imports,
    proc::{Proc, ProcId, ProcSrc},
    subroutine::{LocalSubroutineId, Subroutine, SubroutineSrc, lower_subroutine},
    typedef::{Typedef, TypedefId, lower_typedef_data_ty},
};
use crate::{
    body::{Body, BodySourceMap, OwnerLowering},
    container::InFile,
    db::HirDefDb,
    owner::{OwnerId, OwnerKind},
    region_tree::RegionTree,
    source_map::{DiagnosticSource, Lowered, LoweredData, LoweringDiagnostic, SourceMap},
};

pub mod clocking;
pub mod continuous_assign;
pub mod defparam;
pub mod generate;
pub mod instantiation;
pub mod modport;
pub mod port;
pub mod specify;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Module {
    pub name: Option<Ident>,
    pub items: Vec<ModuleItem>,
    pub param_ports: Option<IdxRange<Declarator>>,
    pub ports: Ports,
    pub cont_assigns: Arena<ContAssign>,
    pub defparams: Arena<DefParam>,
    pub generate_regions: Arena<GenerateRegion>,
    pub specify_blocks: Arena<SpecifyBlock>,
    pub specify_items: Arena<SpecifyItem>,
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
}
impl Module {
    pub fn shrink_to_fit(&mut self) {
        self.ports.shrink_to_fit();
        self.cont_assigns.shrink_to_fit();
        self.defparams.shrink_to_fit();
        self.generate_regions.shrink_to_fit();
        self.specify_blocks.shrink_to_fit();
        self.specify_items.shrink_to_fit();
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
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct ModuleSourceMap {
    pub region_tree: RegionTree,
    pub port_srcs: PortSrcs,
    pub assign_srcs: SourceMap<ContAssign>,
    pub defparam_srcs: SourceMap<DefParam>,
    pub generate_region_srcs: SourceMap<GenerateRegion>,
    pub specify_block_srcs: SourceMap<SpecifyBlock>,
    pub specify_item_srcs: SourceMap<SpecifyItem>,
    pub subroutine_srcs: SourceMap<Subroutine>,
    pub modport_srcs: SourceMap<ModportDef>,
    pub default_clocking_src: Option<DefaultClockingRefSrc>,
    pub clocking_block_srcs: SourceMap<ClockingBlockDef>,
    pub checker_srcs: SourceMap<CheckerDef>,
    pub covergroup_srcs: SourceMap<CovergroupDef>,
    pub coverpoint_srcs: SourceMap<CoverpointDef>,
    pub cross_srcs: SourceMap<CrossDef>,
    pub instantiation_srcs: SourceMap<Instantiation>,
    pub inst_param_assign_srcs: SourceMap<ParamAssign>,
    pub instance_srcs: SourceMap<Instance>,
    pub inst_port_conn_srcs: SourceMap<PortConn>,
    pub proc_srcs: SourceMap<Proc>,
    pub diagnostics: Vec<LoweringDiagnostic>,
}
impl LoweredData for Module {
    type SourceMap = ModuleSourceMap;
}

impl DiagnosticSource for ModuleSourceMap {
    fn diagnostics(&self) -> &[LoweringDiagnostic] {
        &self.diagnostics
    }
}

impl ModuleSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.port_srcs.shrink_to_fit();
        self.assign_srcs.shrink_to_fit();
        self.defparam_srcs.shrink_to_fit();
        self.generate_region_srcs.shrink_to_fit();
        self.specify_block_srcs.shrink_to_fit();
        self.specify_item_srcs.shrink_to_fit();
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
        self.diagnostics.shrink_to_fit();
    }
}

crate::impl_arena_getters!(
    Module;
    NonAnsiPortId => ports => NonAnsiPort,
    PortRefId => ports => PortRef,
    PortDeclId => ports => PortDecl,
    ContAssignId => cont_assigns => ContAssign,
    DefParamId => defparams => DefParam,
    GenerateRegionId => generate_regions => GenerateRegion,
    SpecifyBlockId => specify_blocks => SpecifyBlock,
    SpecifyItemId => specify_items => SpecifyItem,
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
);

crate::impl_source_map_getters!(
    ModuleSourceMap;
    NonAnsiPortId => port_srcs,
    PortRefId => port_srcs,
    PortDeclId => port_srcs,
    ContAssignId => assign_srcs,
    DefParamId => defparam_srcs,
    GenerateRegionId => generate_region_srcs,
    SpecifyBlockId => specify_block_srcs,
    SpecifyItemId => specify_item_srcs,
    LocalSubroutineId => subroutine_srcs,
    ModportId => modport_srcs,
    ClockingBlockId => clocking_block_srcs,
    CheckerId => checker_srcs,
    CovergroupId => covergroup_srcs,
    CoverpointId => coverpoint_srcs,
    CrossId => cross_srcs,
    InstantiationId => instantiation_srcs,
    ParamAssignId => inst_param_assign_srcs,
    InstanceId => instance_srcs,
    PortConnId => inst_port_conn_srcs,
    ProcId => proc_srcs,
);

pub type ModuleSrc = crate::ast_id_map::SourceAstId;

impl Module {
    pub fn param_port_id_by_idx(&self, idx: usize) -> Option<DeclId> {
        self.param_ports.clone()?.nth(idx)
    }

    pub fn overridable_param_id_by_idx(&self, body: &Body, idx: usize) -> Option<DeclId> {
        body.declarations
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
    pub fn item_to_source(
        &self,
        body: &BodySourceMap,
        item: &ModuleItem,
    ) -> Option<crate::ast_id_map::SourceAstId> {
        match item {
            ModuleItem::ContAssignId(idx) => self.get(*idx),
            ModuleItem::DefParamId(idx) => self.get(*idx),
            ModuleItem::GenerateRegionId(idx) => self.get(*idx),
            ModuleItem::SpecifyBlockId(idx) => self.get(*idx),
            ModuleItem::SpecifyItemId(idx) => self.get(*idx),
            ModuleItem::DeclarationId(idx) => body.declaration_srcs.hir_to_src(*idx),
            ModuleItem::StructId(idx) => body.struct_srcs.hir_to_src(*idx),
            ModuleItem::InstantiationId(idx) => self.get(*idx),
            ModuleItem::ProcId(idx) => self.get(*idx),
            ModuleItem::PortDeclId(idx) => self.get(*idx),
            ModuleItem::TypedefId(idx) => body.typedef_srcs.hir_to_src(*idx),
            ModuleItem::SubroutineId(idx) => self.get(*idx),
            ModuleItem::ModportId(idx) => self.get(*idx),
            ModuleItem::ClockingBlockId(idx) => self.get(*idx),
            ModuleItem::CheckerId(idx) => self.get(*idx),
            ModuleItem::CovergroupId(idx) => self.get(*idx),
        }
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

pub(crate) type LowerModuleCtx<'a> = LoweringCtx<ModuleStore<'a>>;

impl LowerModuleCtx<'_> {
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
                        &self.ast_ids,
                        &self.tree,
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
                        &self.ast_ids,
                        &self.tree,
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
            &self.ast_ids,
            &self.tree,
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
                if let Declaration::ParamDecl(param_decl) = &self.store.body.declarations[decl_id] {
                    inherited_kind = param_decl.kind;
                }
                self.region_tree.handle_node(decls.syntax());
            }

            let param_port_range = {
                let mut decls = self.store.body.decls.iter().map(|(id, _)| id);
                decls.next().map(|first| {
                    let last = decls.next_back().unwrap_or(first);
                    IdxRange::new_inclusive(first..=last)
                })
            };
            self.store.data.param_ports = param_port_range;

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
            let idx: ModuleItem = match member {
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
                        let item = ModuleItem::from(modport_id);
                        self.store.data.items.push(item.clone());
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
            self.store.data.items.push(idx.clone());
            self.region_tree.handle_node(member.syntax());
        }
        self.region_tree.stage(decl.endmodule(), decl.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
fn module_lowering(db: &dyn HirDefDb, owner: OwnerId) -> Arc<OwnerLowering<Module>> {
    debug_assert_eq!(owner.kind(db), OwnerKind::Module);
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let mut module = Module::default();
    let mut module_source_map = ModuleSourceMap::default();
    let mut body = Body::default();
    let mut body_source_map = BodySourceMap::default();

    let Some(ast_module) =
        db.ast_id_map(file_id).node(owner.ast_id(db), &tree).and_then(ast::ModuleDeclaration::cast)
    else {
        return Arc::new(OwnerLowering::new(
            file_id,
            module,
            module_source_map,
            body,
            body_source_map,
        ));
    };
    module.name = lower_ident_opt(ast_module.header().name());

    let mut lower_ctx = LoweringCtx::new(
        db,
        owner,
        ModuleStore {
            data: &mut module,
            sources: &mut module_source_map,
            body: &mut body,
            body_sources: &mut body_source_map,
        },
    );
    lower_ctx.lower_module_decl(ast_module);
    let diagnostics = lower_ctx.emit_diagnostics();
    drop(lower_ctx);
    module_source_map.diagnostics = diagnostics.clone();
    body_source_map.diagnostics = diagnostics;

    module.shrink_to_fit();
    module_source_map.shrink_to_fit();
    body.shrink_to_fit();
    body_source_map.shrink_to_fit();
    Arc::new(OwnerLowering::new(file_id, module, module_source_map, body, body_source_map))
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn module_with_source_map(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Module>> {
    module_lowering(db, owner).structure.clone()
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn module_body_with_source_map(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    module_lowering(db, owner).body.clone()
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn module_data(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Module> {
    module_with_source_map(db, owner).data()
}

pub(crate) fn set_module_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    module_lowering::set_lru_capacity(db, capacity);
    module_with_source_map::set_lru_capacity(db, capacity);
    module_body_with_source_map::set_lru_capacity(db, capacity);
    module_data::set_lru_capacity(db, capacity);
}
