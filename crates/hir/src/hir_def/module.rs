use continuous_assgin::{
    ContAssign, ContAssignId, ContAssignSrc, LowerContAssign, impl_lower_cont_assign,
};
use defparam::{DefParam, DefParamId, DefParamSrc, LowerDefParam, impl_lower_defparam};
use generate::{GenerateRegion, GenerateRegionId, GenerateRegionSrc};
use instantiation::{
    Instance, InstanceSrc, Instantiation, InstantiationId, InstantiationSrc, LowerInstantiation,
    ParamAssign, ParamAssignSrc, PortConn, PortConnSrc, impl_lower_instantiation,
};
use la_arena::{Arena, Idx, IdxRange, RawIdx};
use port::{
    NonAnsiPort, NonAnsiPortId, NonAnsiPortSrc, PortDecl, PortDeclId, PortDeclSrc, PortRef,
    PortRefId, PortRefSrc, PortSrcs, Ports,
};
use proc_macro_utils::define_container;
use specify::{
    SpecifyBlock, SpecifyBlockId, SpecifyBlockSrc, SpecifyItem, SpecifyItemId, SpecifyItemSrc,
};
use syntax::{
    ast::{self, AstNode, PortList},
    ptr::SyntaxNodePtr,
};
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use super::{
    HirData, Ident,
    aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
    alloc_idx_and_src,
    block::{BlockInfo, BlockSrc, LocalBlockId},
    declaration::{
        Declaration, DeclarationId, DeclarationSrc, LowerDeclaration, impl_lower_declaration,
    },
    expr::{
        Expr, ExprSrc, LowerExpr,
        declarator::{DeclId, Declarator, DeclaratorSrc, impl_lower_decl},
        impl_lower_expr,
        timing_control::{EventExpr, EventExprSrc, impl_lower_event_expr},
    },
    lower_ident_opt,
    proc::{LowerProc, LowerProcCtx, Proc, ProcId, ProcSrc},
    stmt::{Stmt, StmtId, StmtSrc, impl_lower_stmt},
    subroutine::{
        LocalSubroutineId, LowerSubroutineBodyCtx, Subroutine, SubroutineLoc, SubroutineSrc,
        lower_subroutine, lower_subroutine_body,
    },
    ty::NetKind,
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::{ContainerId, InFile},
    db::{HirDb, InternDb},
    define_src_with_name_and_token,
    file::HirFileId,
    region_tree::{RegionTree, RegionTreeBuilder},
    source_map::{SourceMap, ToAstNode},
};

pub mod continuous_assgin;
pub mod defparam;
pub mod generate;
pub mod instantiation;
pub mod port;
pub mod specify;

define_container! {
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct Module {
        name: Option<Ident>,

        param_ports: Option<IdxRange<Declarator>>,
        ports: Ports => {
            [NonAnsiPortId | NonAnsiPort],
            [PortRefId | PortRef],
            [PortDeclId | PortDecl],
        },

        cont_assigns: [ContAssign],
        defparams: [DefParam],
        generate_regions: [GenerateRegion],
        specify_blocks: [SpecifyBlock],
        specify_items: [SpecifyItem],
        declarations: [Declaration],
        typedefs: [Typedef],
        structs: [StructDef],
        subroutines: [Subroutine],

        instantiations: [Instantiation],
        inst_param_assigns: [ParamAssign],
        instances: [Instance],
        inst_port_conns: [PortConn],

        procs: [Proc],

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
    pub struct ModuleSourceMap {
        items: Vec<ModuleItem>,
        region_tree: RegionTree,

        port_srcs: PortSrcs => {
            [NonAnsiPortId | NonAnsiPortSrc],
            [PortRefId | PortRefSrc],
            [PortDeclId | PortDeclSrc],
        },

        assign_srcs: [ContAssign | ContAssignSrc],
        defparam_srcs: [DefParam | DefParamSrc],
        generate_region_srcs: [GenerateRegion | GenerateRegionSrc],
        specify_block_srcs: [SpecifyBlock | SpecifyBlockSrc],
        specify_item_srcs: [SpecifyItem | SpecifyItemSrc],
        declaration_srcs: [Declaration | DeclarationSrc],
        typedef_srcs: [Typedef | TypedefSrc],
        struct_srcs: [StructDef | StructSrc],
        subroutine_srcs: [Subroutine | SubroutineSrc],

        instantiation_srcs: [Instantiation | InstantiationSrc],
        inst_param_assign_srcs: [ParamAssign | ParamAssignSrc],
        instance_srcs: [Instance | InstanceSrc],
        inst_port_conn_srcs: [PortConn | PortConnSrc],

        proc_srcs: [Proc | ProcSrc],

        expr_srcs: [Expr | ExprSrc],
        event_expr_srcs: [EventExpr | EventExprSrc],
        decl_srcs: [Declarator | DeclaratorSrc],
        stmt_srcs: [Stmt | StmtSrc] => {
            [StmtId | StmtSrc],
            [LocalBlockId | BlockSrc],
        },
    }
}

define_src_with_name_and_token!(ModuleSrc(ast::ModuleDeclaration, end: endmodule, end_range));

impl Module {
    pub fn param_port_id_by_idx(&self, idx: usize) -> Option<DeclId> {
        self.param_ports.clone()?.nth(idx)
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
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ModuleInfo {
    pub name: Option<Ident>,
}

pub type LocalModuleId = Idx<ModuleInfo>;
pub type ModuleId = InFile<LocalModuleId>;

pub(crate) struct LowerModuleCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) module_id: ModuleId,
    pub(crate) default_net_type: NetKind,

    pub(crate) module: &'a mut Module,
    pub(crate) module_source_map: &'a mut ModuleSourceMap,
    pub(crate) region_tree: RegionTreeBuilder,
}

impl_lower_expr!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_decl!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_event_expr!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_stmt!(LowerModuleCtx<'_>, module_id, module, module_source_map);
impl_lower_declaration!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_cont_assign!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_defparam!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_instantiation!(LowerModuleCtx<'_>, module, module_source_map);

impl LowerProc for LowerModuleCtx<'_> {
    fn proc_ctx(&mut self) -> LowerProcCtx<'_> {
        LowerProcCtx {
            db: self.db,
            file_id: self.file_id,
            cont_id: self.module_id.into(),

            procs: &mut self.module.procs,
            proc_srcs: &mut self.module_source_map.proc_srcs,

            stmts: &mut self.module.stmts,
            stmt_srcs: &mut self.module_source_map.stmt_srcs,

            exprs: &mut self.module.exprs,
            expr_srcs: &mut self.module_source_map.expr_srcs,

            event_exprs: &mut self.module.event_exprs,
            event_expr_srcs: &mut self.module_source_map.event_expr_srcs,

            decls: &mut self.module.decls,
            decl_srcs: &mut self.module_source_map.decl_srcs,
        }
    }
}

impl LowerModuleCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = ContainerId::ModuleId(self.module_id);
        let struct_def =
            lower_struct_def(struct_ty, container_id, |ty| self.expr_ctx().lower_data_ty(ty));

        alloc_idx_and_src! {
            struct_def => self.module.structs,
            struct_ty => self.module_source_map.struct_srcs,
        }
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());

        let typedef_id = alloc_idx_and_src! {
            Typedef { name, ty: None } => self.module.typedefs,
            typedef => self.module_source_map.typedef_srcs,
        };

        let data_ty = typedef.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            ContainerId::ModuleId(self.module_id),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.expr_ctx().lower_data_ty(ty),
        );

        self.module.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_subroutine_decl(
        &mut self,
        func: ast::FunctionDeclaration,
    ) -> Option<LocalSubroutineId> {
        let subroutine = lower_subroutine(&func, |ty| self.expr_ctx().lower_data_ty(ty))?;

        let subroutine_id = alloc_idx_and_src! {
            subroutine => self.module.subroutines,
            func => self.module_source_map.subroutine_srcs,
        };

        let src = SubroutineSrc::from(func);
        let subroutine_def_id = self.db.intern_subroutine(SubroutineLoc {
            cont_id: self.module_id.into(),
            src: InFile::new(self.file_id, src),
            local_id: subroutine_id,
        });

        if func.end().is_some() {
            let subroutine = &mut self.module.subroutines[subroutine_id];
            let mut subroutine_source_map = std::mem::take(&mut subroutine.source_map);
            let mut ctx = LowerSubroutineBodyCtx {
                db: self.db,
                file_id: self.file_id,
                subroutine_id: subroutine_def_id,
                subroutine,
                subroutine_source_map: &mut subroutine_source_map,
                region_tree: RegionTreeBuilder::new(),
            };
            lower_subroutine_body(&mut ctx, func);
            subroutine.source_map = subroutine_source_map;
            subroutine.source_map.shrink_to_fit();
        }

        self.module.subroutines[subroutine_id].shrink_to_fit();

        Some(subroutine_id)
    }

    pub(crate) fn lower_module_decl(&mut self, decl: ast::ModuleDeclaration) {
        let header = decl.header();
        if let Some(param_ports) = header.parameters() {
            for decls in param_ports.declarations().children() {
                self.declaration_ctx().lower_param_decl_base(decls);
                self.region_tree.handle_node(decls.syntax());
            }

            let beg = Idx::from_raw(RawIdx::from(0));
            let end = self.module.decls.nxt_idx();
            if beg != end {
                self.module.param_ports = Some(IdxRange::new(beg..end));
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
                ContinuousAssign(assign) => {
                    self.cont_assign_ctx().lower_continuous_assign(assign).into()
                }

                // Declarations
                DataDeclaration(data_decl) => {
                    self.declaration_ctx().lower_data_decl(data_decl).into()
                }
                NetDeclaration(net_decl) => self.declaration_ctx().lower_net_decl(net_decl).into(),
                LocalVariableDeclaration(_) => continue,
                ParameterDeclarationStatement(param_decl) => {
                    self.declaration_ctx().lower_param_decl_base(param_decl.parameter()).into()
                }
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                GenvarDeclaration(genvar_decl) => {
                    self.declaration_ctx().lower_genvar_decl(genvar_decl).into()
                }
                NetTypeDeclaration(_)
                | ForwardTypedefDeclaration(_)
                | UserDefinedNetDeclaration(_) => {
                    continue;
                }

                // Instantiations
                HierarchyInstantiation(instantiation) => {
                    self.instantiation_ctx().lower_instantiation(instantiation).into()
                }
                PrimitiveInstantiation(instantiation) => {
                    self.instantiation_ctx().lower_primitive_instantiation(instantiation).into()
                }
                CheckerInstantiation(_) => continue,

                // Subroutines
                FunctionDeclaration(fn_decl) => match self.lower_subroutine_decl(fn_decl) {
                    Some(sub_id) => sub_id.into(),
                    None => continue,
                },

                // Procedural blocks
                ProceduralBlock(proc) => self.proc_ctx().lower_proc(proc).into(),

                // Ports
                PortDeclaration(port) => self.lower_port_decl(port).into(),
                ExplicitAnsiPort(_) | ImplicitAnsiPort(_) => continue,

                // Imports
                PackageImportDeclaration(_) => continue,

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
                TimeUnitsDeclaration(_)
                | ClockingDeclaration(_)
                | DefaultClockingReference(_)
                | ClockingItem(_) => continue,

                // Assertions and properties
                PropertyDeclaration(_)
                | SequenceDeclaration(_)
                | ImmediateAssertionMember(_)
                | ConcurrentAssertionMember(_) => continue,

                // Coverage
                CovergroupDeclaration(_)
                | Coverpoint(_)
                | CoverCross(_)
                | CoverageBins(_)
                | BinsSelection(_)
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
                    self.declaration_ctx().lower_specparam_decl(specparam_decl).into()
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
                DefParam(defparam) => self.defparam_ctx().lower_defparam(defparam).into(),

                // Net alias
                NetAlias(_) => continue,

                // Modport
                ModportDeclaration(_)
                | ModportClockingPort(_)
                | ModportSimplePortList(_)
                | ModportSubroutinePortList(_) => continue,

                // Class members (shouldn't appear in module but handle anyway)
                ClassPropertyDeclaration(_)
                | ClassMethodDeclaration(_)
                | ClassMethodPrototype(_) => continue,

                // Checker
                CheckerDeclaration(_) | CheckerDataDeclaration(_) => continue,

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
            self.module_source_map.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }
        self.region_tree.stage(decl.endmodule(), decl.syntax());
        self.module_source_map.region_tree = self.region_tree.finish();
    }
}

pub(crate) fn module_with_source_map_query(
    db: &dyn HirDb,
    module_id @ InFile { value: local_module_id, file_id }: ModuleId,
) -> (Arc<Module>, Arc<ModuleSourceMap>) {
    let (file, file_source_map) = db.hir_file_with_source_map(file_id);
    let tree = db.parse(file_id);

    let mut module = Module { name: file.get(local_module_id).name.clone(), ..Default::default() };
    let mut module_source_map = ModuleSourceMap::default();

    let Some(ast_module) = file_source_map.get(local_module_id).and_then(|src| src.to_node(&tree))
    else {
        return (Arc::new(module), Arc::new(module_source_map));
    };

    let mut lower_ctx = LowerModuleCtx {
        db,
        default_net_type: NetKind::Wire,
        file_id,
        module_id,
        module: &mut module,
        module_source_map: &mut module_source_map,
        region_tree: RegionTreeBuilder::new(),
    };
    lower_ctx.lower_module_decl(ast_module);

    module.shrink_to_fit();
    module_source_map.shrink_to_fit();
    (Arc::new(module), Arc::new(module_source_map))
}
