use continuous_assgin::{ContAssign, ContAssignId, ContAssignSrc};
use instantiation::{
    Instance, InstanceSrc, Instantiation, InstantiationId, InstantiationSrc, ParamAssign,
    ParamAssignSrc, PortConn, PortConnSrc,
};
use la_arena::{Arena, Idx, IdxRange, RawIdx};
use port::{
    NonAnsiPort, NonAnsiPortId, NonAnsiPortSrc, PortDecl, PortDeclId, PortDeclSrc, PortRef,
    PortRefId, PortRefSrc, PortSrcs, Ports,
};
use proc_macro_utils::define_container;
use rustc_hash::FxHashMap;
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
        LocalSubroutineId, LowerSubroutineBodyCtx, Subroutine, SubroutineLoc, SubroutineSourceMap,
        SubroutineSrc, lower_subroutine, lower_subroutine_body,
    },
    ty::NetKind,
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::{ContainerId, InFile},
    db::{HirDb, InternDb},
    define_src_with_name,
    file::HirFileId,
    region_tree::{RegionTree, RegionTreeBuilder},
    source_map::{SourceMap, ToAstNode},
};

pub mod continuous_assgin;
pub mod instantiation;
pub mod port;

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
        declarations: [Declaration],
        typedefs: [Typedef],
        structs: [StructDef],
        subroutines: [Subroutine],
        subroutine_source_maps: FxHashMap<LocalSubroutineId, SubroutineSourceMap>,

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

define_src_with_name!(ModuleSrc(ast::ModuleDeclaration));

impl Module {
    pub fn param_port_id_by_idx(&self, idx: usize) -> Option<DeclId> {
        let start = self.param_ports.as_ref()?.start();
        let raw_idx = (start.into_raw().into_u32() as usize) + idx;
        Some(Idx::from_raw(RawIdx::from_u32(raw_idx as u32)))
    }

    pub fn non_ansi_port_id_by_idx(&self, idx: usize) -> NonAnsiPortId {
        Idx::from_raw(RawIdx::from_u32(idx as u32))
    }

    pub fn ansi_port_id_by_idx(&self, idx: usize) -> Option<DeclId> {
        let Ports::Ansi(decls) = &self.ports else {
            return None;
        };

        let start = decls.values().next()?.decls.start();
        let raw_idx = (start.into_raw().into_u32() as usize) + idx;
        if raw_idx > decls.len() {
            return None;
        }
        Some(Idx::from_raw(RawIdx::from_u32(raw_idx as u32)))
    }
}

impl ModuleSourceMap {
    pub fn item_to_ptr(&self, item: &ModuleItem) -> SyntaxNodePtr {
        match item {
            ModuleItem::ContAssignId(idx) => self.get(*idx).0,
            ModuleItem::DeclarationId(idx) => self.get(*idx).ptr(),
            ModuleItem::StructId(idx) => self.get(*idx).node,
            ModuleItem::InstantiationId(idx) => self.get(*idx).into(),
            ModuleItem::ProcId(idx) => self.get(*idx).0,
            ModuleItem::PortDeclId(idx) => self.get(*idx).ptr(),
            ModuleItem::TypedefId(idx) => self.get(*idx).ptr(),
            ModuleItem::SubroutineId(idx) => self.get(*idx).node,
        }
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum ModuleItem {
        ContAssignId(ContAssignId),
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
    pub(crate) default_net_type: Option<NetKind>,

    pub(crate) module: &'a mut Module,
    pub(crate) module_source_map: &'a mut ModuleSourceMap,
    pub(crate) region_tree: RegionTreeBuilder,
}

impl_lower_expr!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_decl!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_event_expr!(LowerModuleCtx<'_>, module, module_source_map);
impl_lower_stmt!(LowerModuleCtx<'_>, module_id, module, module_source_map);
impl_lower_declaration!(LowerModuleCtx<'_>, module, module_source_map);

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
        let src = SubroutineSrc::from(func);
        let subroutine_def_id = self.db.intern_subroutine(SubroutineLoc {
            cont_id: self.module_id.into(),
            src: InFile::new(self.file_id, src),
        });

        let subroutine = lower_subroutine(&func, |ty| self.expr_ctx().lower_data_ty(ty))?;

        let subroutine_id = alloc_idx_and_src! {
            subroutine => self.module.subroutines,
            func => self.module_source_map.subroutine_srcs,
        };

        if func.end().is_some() {
            let subroutine = &mut self.module.subroutines[subroutine_id];
            let mut subroutine_source_map = SubroutineSourceMap::default();
            let mut ctx = LowerSubroutineBodyCtx {
                db: self.db,
                file_id: self.file_id,
                subroutine_id: subroutine_def_id,
                subroutine,
                subroutine_source_map: &mut subroutine_source_map,
                region_tree: RegionTreeBuilder::new(),
            };
            lower_subroutine_body(&mut ctx, func);
            self.module.subroutine_source_maps.insert(subroutine_id, subroutine_source_map);
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

            self.region_tree.stage(param_ports.close_paren());
        }

        match header.ports() {
            Some(PortList::AnsiPortList(port_list)) => self.lower_ansi_ports(port_list),
            Some(PortList::NonAnsiPortList(port_list)) => self.lower_nonansi_port(port_list),
            Some(PortList::WildcardPortList(_)) => unimplemented!(),
            None => {}
        };

        for member in decl.members().children() {
            use ast::Member::*;
            let idx = match member {
                // Assignments
                ContinuousAssign(assign) => self.lower_continuous_assign(assign).into(),

                // Declarations
                DataDeclaration(data_decl) => {
                    self.declaration_ctx().lower_data_decl(data_decl).into()
                }
                NetDeclaration(net_decl) => self.declaration_ctx().lower_net_decl(net_decl).into(),
                LocalVariableDeclaration(_local_decl) => {
                    // Local variable declarations shouldn't appear at module level,
                    // they appear in function/task bodies
                    continue;
                }
                ParameterDeclarationStatement(param_decl) => {
                    self.declaration_ctx().lower_param_decl_base(param_decl.parameter()).into()
                }
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                NetTypeDeclaration(_net_type_decl) => {
                    // TODO: implement net type declaration lowering
                    continue;
                }
                ForwardTypedefDeclaration(_fwd_typedef) => {
                    // TODO: implement forward typedef lowering
                    continue;
                }
                UserDefinedNetDeclaration(_udn_decl) => {
                    // TODO: implement user defined net declaration lowering
                    continue;
                }
                GenvarDeclaration(_genvar_decl) => {
                    // TODO: implement genvar declaration lowering
                    continue;
                }

                // Instantiations
                HierarchyInstantiation(instantiation) => {
                    self.lower_instantiation(instantiation).into()
                }
                PrimitiveInstantiation(instantiation) => {
                    self.lower_primitive_instantiation(instantiation).into()
                }
                CheckerInstantiation(_checker_inst) => {
                    // TODO: implement checker instantiation lowering
                    continue;
                }

                // Subroutines
                FunctionDeclaration(fn_decl) => match self.lower_subroutine_decl(fn_decl) {
                    Some(sub_id) => sub_id.into(),
                    None => continue,
                },

                // Procedural blocks
                ProceduralBlock(proc) => self.proc_ctx().lower_proc(proc).into(),

                // Ports
                PortDeclaration(port) => self.lower_port_decl(port).into(),
                ExplicitAnsiPort(_) | ImplicitAnsiPort(_) => unreachable!(),

                // Imports
                PackageImportDeclaration(_) => {
                    // TODO: implement package import declaration lowering
                    continue;
                }

                // Aggregates
                ClassDeclaration(_) => {
                    // TODO: implement class declaration lowering
                    continue;
                }

                // Nested modules/interfaces/programs
                ModuleDeclaration(_nested_module) => {
                    // TODO: handle nested module declarations
                    continue;
                }

                // Generate constructs
                GenerateRegion(_gen_region) => {
                    // TODO: implement generate region lowering
                    continue;
                }
                GenerateBlock(_gen_block) => {
                    // TODO: implement generate block lowering
                    continue;
                }
                IfGenerate(_if_gen) => {
                    // TODO: implement if generate lowering
                    continue;
                }
                CaseGenerate(_case_gen) => {
                    // TODO: implement case generate lowering
                    continue;
                }
                LoopGenerate(_loop_gen) => {
                    // TODO: implement loop generate lowering
                    continue;
                }

                // Timing and clocking
                TimeUnitsDeclaration(_time_units) => {
                    // TODO: implement time units declaration lowering
                    continue;
                }
                ClockingDeclaration(_clocking_decl) => {
                    // TODO: implement clocking declaration lowering
                    continue;
                }
                DefaultClockingReference(_default_clocking) => {
                    // TODO: implement default clocking reference lowering
                    continue;
                }
                ClockingItem(_clocking_item) => {
                    // TODO: implement clocking item lowering
                    continue;
                }

                // Assertions and properties
                PropertyDeclaration(_prop_decl) => {
                    // TODO: implement property declaration lowering
                    continue;
                }
                SequenceDeclaration(_seq_decl) => {
                    // TODO: implement sequence declaration lowering
                    continue;
                }
                ImmediateAssertionMember(_assert_member) => {
                    // TODO: implement immediate assertion lowering
                    continue;
                }
                ConcurrentAssertionMember(_assert_member) => {
                    // TODO: implement concurrent assertion lowering
                    continue;
                }

                // Coverage
                CovergroupDeclaration(_covergroup) => {
                    // TODO: implement covergroup lowering
                    continue;
                }
                Coverpoint(_coverpoint) => {
                    // TODO: implement coverpoint lowering
                    continue;
                }
                CoverCross(_cover_cross) => {
                    // TODO: implement cover cross lowering
                    continue;
                }
                CoverageBins(_coverage_bins) => {
                    // TODO: implement coverage bins lowering
                    continue;
                }
                BinsSelection(_bins_selection) => {
                    // TODO: implement bins selection lowering
                    continue;
                }
                CoverageOption(_coverage_option) => {
                    // TODO: implement coverage option lowering
                    continue;
                }

                // Specify blocks
                SpecifyBlock(_specify_block) => {
                    // TODO: implement specify block lowering
                    continue;
                }
                PathDeclaration(_path_decl) => {
                    // TODO: implement path declaration lowering
                    continue;
                }
                ConditionalPathDeclaration(_cond_path) => {
                    // TODO: implement conditional path declaration lowering
                    continue;
                }
                IfNonePathDeclaration(_if_none_path) => {
                    // TODO: implement if none path declaration lowering
                    continue;
                }
                SystemTimingCheck(_timing_check) => {
                    // TODO: implement system timing check lowering
                    continue;
                }
                SpecparamDeclaration(_specparam) => {
                    // TODO: implement specparam declaration lowering
                    continue;
                }
                PulseStyleDeclaration(_pulse_style) => {
                    // TODO: implement pulse style declaration lowering
                    continue;
                }
                DefaultSkewItem(_default_skew) => {
                    // TODO: implement default skew item lowering
                    continue;
                }

                // DPI and external
                DPIImport(_dpi_import) => {
                    // TODO: implement DPI import lowering
                    continue;
                }
                DPIExport(_dpi_export) => {
                    // TODO: implement DPI export lowering
                    continue;
                }
                ExternInterfaceMethod(_extern_method) => {
                    // TODO: implement extern interface method lowering
                    continue;
                }
                ExternModuleDecl(_extern_module) => {
                    // TODO: implement extern module declaration lowering
                    continue;
                }
                ExternUdpDecl(_extern_udp) => {
                    // TODO: implement extern UDP declaration lowering
                    continue;
                }

                // UDP
                UdpDeclaration(_udp_decl) => {
                    // TODO: implement UDP declaration lowering
                    continue;
                }

                // Defparam
                DefParam(_defparam) => {
                    // TODO: implement defparam lowering
                    continue;
                }

                // Net alias
                NetAlias(_net_alias) => {
                    // TODO: implement net alias lowering
                    continue;
                }

                // Modport
                ModportDeclaration(_modport_decl) => {
                    // TODO: implement modport declaration lowering
                    continue;
                }
                ModportClockingPort(_modport_clocking) => {
                    // TODO: implement modport clocking port lowering
                    continue;
                }
                ModportSimplePortList(_modport_simple) => {
                    // TODO: implement modport simple port list lowering
                    continue;
                }
                ModportSubroutinePortList(_modport_subroutine) => {
                    // TODO: implement modport subroutine port list lowering
                    continue;
                }

                // Class members (shouldn't appear in module but handle anyway)
                ClassPropertyDeclaration(_class_prop) => {
                    // Class property shouldn't appear directly in module
                    continue;
                }
                ClassMethodDeclaration(_class_method) => {
                    // Class method shouldn't appear directly in module
                    continue;
                }
                ClassMethodPrototype(_class_method_proto) => {
                    // Class method prototype shouldn't appear directly in module
                    continue;
                }

                // Checker
                CheckerDeclaration(_checker_decl) => {
                    // TODO: implement checker declaration lowering
                    continue;
                }
                CheckerDataDeclaration(_checker_data) => {
                    // TODO: implement checker data declaration lowering
                    continue;
                }

                // Constraints
                ConstraintDeclaration(_constraint_decl) => {
                    // TODO: implement constraint declaration lowering
                    continue;
                }
                ConstraintPrototype(_constraint_proto) => {
                    // TODO: implement constraint prototype lowering
                    continue;
                }

                // Config
                ConfigDeclaration(_config_decl) => {
                    // TODO: implement config declaration lowering
                    continue;
                }

                // Bind
                BindDirective(_bind_directive) => {
                    // TODO: implement bind directive lowering
                    continue;
                }

                // Package exports
                PackageExportDeclaration(_pkg_export) => {
                    // TODO: implement package export declaration lowering
                    continue;
                }
                PackageExportAllDeclaration(_pkg_export_all) => {
                    // TODO: implement package export all declaration lowering
                    continue;
                }

                // Library
                LibraryDeclaration(_lib_decl) => {
                    // TODO: implement library declaration lowering
                    continue;
                }
                LibraryIncludeStatement(_lib_include) => {
                    // TODO: implement library include statement lowering
                    continue;
                }

                // Let declaration
                LetDeclaration(_let_decl) => {
                    // TODO: implement let declaration lowering
                    continue;
                }

                // Default disable
                DefaultDisableDeclaration(_default_disable) => {
                    // TODO: implement default disable declaration lowering
                    continue;
                }

                // Elaboration system task
                ElabSystemTask(_elab_task) => {
                    // TODO: implement elaboration system task lowering
                    continue;
                }

                // Anonymous program
                AnonymousProgram(_anon_program) => {
                    // TODO: implement anonymous program lowering
                    continue;
                }

                // Empty member - skip
                EmptyMember(_) => continue,
            };
            self.module_source_map.items.push(idx);
            self.region_tree.handle_node(member.syntax());
        }
        self.region_tree.stage(decl.endmodule());
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

    let Some(ast_module) = file_source_map.get(local_module_id).to_node(&tree) else {
        return (Arc::new(module), Arc::new(module_source_map));
    };

    let mut lower_ctx = LowerModuleCtx {
        db,
        default_net_type: Some(NetKind::Wire),
        file_id,
        module_id,
        module: &mut module,
        module_source_map: &mut module_source_map,
        region_tree: RegionTreeBuilder::new(),
    };
    lower_ctx.lower_module_decl(ast_module);

    module.subroutine_source_maps.shrink_to_fit();
    module.shrink_to_fit();
    module_source_map.shrink_to_fit();
    (Arc::new(module), Arc::new(module_source_map))
}
