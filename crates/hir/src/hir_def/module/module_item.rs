use crate::{
    file::InFile,
    hir_def::{
        block::{block_src::LocalBlockSrc, BlockInfo},
        control::{DelayControl, EventExpr, LowerDelayControl, LowerEventExpr, LowerTimingControl},
        data::{self, Delay, Dimension, DriveStrength, LowerDelay, LowerDimension},
        expr::{AssignOp, ExprId, LowerExpr},
        lower::Lower,
        module::{
            lower::ModuleLowerCtx,
            port::{LowerPortDecl, PortDecl},
        },
        pack_or_gen_item::{LowerPackOrGenItemDecl, PackOrGenItemDecl},
        stmt::{Assign, LowerStmt, Stmt, StmtId, StmtSrc},
        try_match, Ident, SourceMap,
    },
};
use la_arena::{Arena, Idx, IdxRange, RawIdx};
use smallvec::SmallVec;
use syntax::ast::{self, ptr};
use utils::try_;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ModuleItem {
    PortDecl(Idx<PortDecl>),
    PackOrGenItemDecl(PackOrGenItemDecl),
    ModuleInst(Idx<Inst>),
    ContinuousAssignment(ContinuousAssignment),
    ProcessConstruct(ProcessConstruct),
    // TODO: Add more module items
    // ParamOverride(Idx<ParamOverride>),
    // InterfaceInstantiation(Idx<InterfaceInstantiation>),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum LocalModuleItemSrc {
    NonePortItem(ptr::NonPortModuleItemPtr),
    PortDecl(ptr::PortDeclarationPtr),
}

pub type ModuleItemSrc = InFile<LocalModuleItemSrc>;

// #[derive(Debug, PartialEq, Eq, Clone, Hash)]
// pub struct ParamOverride {
//     pub hierarchical_ident: HierarchicalIdent,
//     pub expr: NodeId,
//     pub node_id: NodeId,
// }

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Inst {
    pub ident: Ident,
    pub param_assigns: Option<ParamAssigns>,
    pub hierarchical_insts: IdxRange<HierarchicalInst>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ParamAssigns {
    Ordered(SmallVec<[ExprId; 1]>),
    Named(SmallVec<[(Ident, Option<ExprId>); 1]>),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct HierarchicalInst {
    pub ident: Ident,
    pub dimensions: Option<SmallVec<[Dimension; 1]>>,
    pub port_connects: Option<PortConnects>,
    pub full_decl: Idx<Inst>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PortConnects {
    Ordered(SmallVec<[Option<ExprId>; 1]>),
    Named(SmallVec<[(Ident, Option<ExprId>); 1]>),
}

// TODO: net: [drive_strength][delay3] variable: [delay_control]
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ContinuousAssignment {
    Net {
        drive_strength: Option<DriveStrength>,
        delay: Option<Delay>,
        assigns: SmallVec<[Assign; 1]>,
    },
    Var {
        delay_control: Option<DelayControl>,
        assigns: SmallVec<[Assign; 1]>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AlwaysKeyword {
    Always,
    AlwaysComb,
    AlwaysLatch,
    AlwaysFf,
}

pub(crate) fn lower_always_keyword(keyword: &ast::AlwaysKeyword) -> Option<AlwaysKeyword> {
    try_match! {
        keyword.token_always(), _ => {
            Some(AlwaysKeyword::Always)
        },
        keyword.token_always_comb(), _ => {
            Some(AlwaysKeyword::AlwaysComb)
        },
        keyword.token_always_latch(), _ => {
            Some(AlwaysKeyword::AlwaysLatch)
        },
        keyword.token_always_ff(), _ => {
            Some(AlwaysKeyword::AlwaysFf)
        },
        _ => None,
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ProcessType {
    Initial,
    Always(AlwaysKeyword),
    Final,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ProcessConstruct {
    pub process_type: ProcessType,
    pub stmt: Option<StmtId>,
}

impl<'a> ModuleLowerCtx<'a> {
    pub(crate) fn lower_module_item(&mut self, item: &ast::ModuleItem) {
        try_! {
            try_match! {
                item.port_declaration(), port_decl => {
                    let module_item = ModuleItem::PortDecl(self.lower_port_decl(&port_decl)?);
                    let src = self.in_file(LocalModuleItemSrc::PortDecl(port_decl.to_ptr()));
                    let idx = self.module_decl.module_items.alloc(module_item);
                    self.module_src_map.module_item.insert(src, idx);
                },
                item.non_port_module_item(), non_port_item => {
                    self.lower_non_port_module_item(&non_port_item);
                },
                _ => { return None; }
            };
        };
    }

    pub(crate) fn lower_non_port_module_item(&mut self, item: &ast::NonPortModuleItem) {
        try_! {
            let module_item = try_match! {
                item.module_or_generate_item(), module_or_generate_item => {
                    self.lower_module_or_gen_item(&module_or_generate_item)?
                },
                item.generate_region(), _generate_region => {
                    unimplemented!("generate_region")
                },
                item.specify_block(), _ => {
                    unimplemented!("specify_block")
                },
                item.specparam_declaration(), _ => {
                    unimplemented!("specparam_declaration")
                },
                item.program_declaration(), _ => {
                    unimplemented!("program_declaration")
                },
                item.module_declaration(), _ => {
                    unimplemented!("module_declaration")
                },
                item.interface_declaration(), _ => {
                    unimplemented!("interface_declaration")
                },
                item.timeunits_declaration(), _ => {
                    unimplemented!("timeunits_declaration")
                },
                _ => { return None; }
            };
            let src = self.in_file(LocalModuleItemSrc::NonePortItem(item.to_ptr()));
            let idx = self.module_decl.module_items.alloc(module_item);
            self.module_src_map.module_item.insert(src, idx);
        };
    }

    pub(crate) fn lower_module_or_gen_item(
        &mut self,
        item: &ast::ModuleOrGenerateItem,
    ) -> Option<ModuleItem> {
        let module_item = try_match! {
            item.module_common_item(), module_common_item => {
                self.lower_module_common_item(&module_common_item)?
            },
            item.instantiation(), inst => {
                self.lower_instantiation(&inst)?
            },
            item.parameter_override(), _param_override => {
                unimplemented!("parameter_override")
            },
            item.gate_instantiation(), _gate_instantiation => {
                unimplemented!("gate_instantiation")
            },
            item.udp_instantiation(), _udp_instantiation => {
                unimplemented!("udp_instantiation")
            },
            _ => { return None; }
        };
        Some(module_item)
    }

    pub(crate) fn lower_module_common_item(
        &mut self,
        item: &ast::ModuleCommonItem,
    ) -> Option<ModuleItem> {
        let module_item = try_match! {
            item.module_or_generate_item_declaration(), module_or_generate_item_decl => {
                self.lower_module_or_gen_item_decl(&module_or_generate_item_decl)?
            },
            item.continuous_assign(), continuous_assign_node => {
                self.lower_continuous_assign(&continuous_assign_node)?
            },
            item.initial_construct(), init_construct => {
                ModuleItem::ProcessConstruct(ProcessConstruct{
                    process_type: ProcessType::Initial,
                    stmt: self.lower_stmt_or_null(&init_construct.statement_or_null()?)
                })
            },
            item.always_construct(), always_construct => {
                let always_keyword = lower_always_keyword(&always_construct.always_keyword()?)?;
                ModuleItem::ProcessConstruct(ProcessConstruct{
                    process_type: ProcessType::Always(always_keyword),
                    stmt: self.lower_stmt(&always_construct.statement()?)
                })
            },
            item.final_construct(), final_construct => {
                ModuleItem::ProcessConstruct(ProcessConstruct{
                    process_type: ProcessType::Final,
                    stmt: Some(self.lower_stmt(&final_construct.function_statement()?.statement()?)?)
                })
            },
            item.instantiation(), inst => {
                self.lower_instantiation(&inst)?
            },
            item.assertion_item(), _ => {
                unimplemented!("assertion_item")
            },
            item.bind_directive(), _ => {
                unimplemented!("bind_directive")
            },
            item.net_alias(), _ => {
                unimplemented!("net_alias")
            },
            item.loop_generate_construct(), _ => {
                unimplemented!("loop_generate_construct")
            },
            item.conditional_generate_construct(), _ => {
                unimplemented!("conditional_generate_construct")
            },
            item.elaboration_system_task(), _ => {
                unimplemented!("elaboration_system_task")
            },
            _ => { return None;}
        };
        Some(module_item)
    }

    pub(crate) fn lower_module_or_gen_item_decl(
        &mut self,
        item: &ast::ModuleOrGenerateItemDeclaration,
    ) -> Option<ModuleItem> {
        let module_item = try_match! {
            item.package_or_generate_item_declaration(), pack_or_gen_item_decl => {
                ModuleItem::PackOrGenItemDecl(
                    self.lower_pack_or_gen_item_decl(&pack_or_gen_item_decl)?
                )
            },
            item.genvar_declaration(), _genvar_decl => {
                unimplemented!("genvar_declaration");
            },
            item.clocking_declaration(), _clocking_decl => {
                unimplemented!("clocking_declaration");
            },
            item.token_clocking(), _ => {
                unimplemented!("... ::= default clocking identifier");
            },
            item.token_disable(), _ => {
                unimplemented!("... ::= default disable iff expression_or_dist");
            },
            _ => { return None; },
        };
        Some(module_item)
    }

    fn lower_instantiation(&mut self, module_inst: &ast::Instantiation) -> Option<ModuleItem> {
        let ident = self.lower_ident(&module_inst.identifier()?)?;
        let param_assigns = try_! {
            let param_value_assigns = module_inst.parameter_value_assignment()?;
            let param_assigns_node = param_value_assigns.list_of_parameter_assignments()?;
            try_match! {
                param_assigns_node.list_of_ordered_parameter_assignments(), ordered => {
                    let mut assigns: SmallVec<[ExprId; 1]> = SmallVec::new();
                    for assign in ordered.ordered_parameter_assignments() {
                        assigns.push(self.lower_param_expr(&assign.param_expression()?)?)
                    }
                    ParamAssigns::Ordered(assigns)
                },
                param_assigns_node.list_of_named_parameter_assignments(), named => {
                    let mut assigns: SmallVec<[(Ident, Option<ExprId>); 1]> = SmallVec::new();
                    for assign in named.named_parameter_assignments() {
                        let ident = self.lower_ident(&assign.identifier()?)?;
                        let expr = assign.param_expression().map(|param_expr| self.lower_param_expr(&param_expr))?;
                        assigns.push((ident, expr))
                    }
                    ParamAssigns::Named(assigns)
                },
                _ => { return None; }
            }
        };
        let module_inst_src = self.in_file(module_inst.to_ptr());
        let module_inst_idx = Idx::from_raw(RawIdx::from(self.module_decl.data.insts.len() as u32));

        let begin_idx = self.module_decl.data.hierarchical_insts.len();
        let begin_idx = Idx::from_raw(RawIdx::from(begin_idx as u32));
        for instance_node in module_inst.hierarchical_instances() {
            try_! {
                let instance = self.lower_hierarchy_instance(&instance_node, module_inst_idx)?;
                let src = self.in_file(instance_node.to_ptr());
                let idx = self.module_decl.data.hierarchical_insts.alloc(instance);
                self.module_src_map.hierarchical_inst.insert(src, idx);
            };
        }
        let end_idx = self.module_decl.data.hierarchical_insts.len();
        let end_idx = Idx::from_raw(RawIdx::from(end_idx as u32));
        let hierarchical_insts = IdxRange::new(begin_idx..end_idx);

        self.module_decl.data.insts.alloc(Inst { ident, param_assigns, hierarchical_insts });
        self.module_src_map.inst.insert(module_inst_src, module_inst_idx);

        Some(ModuleItem::ModuleInst(module_inst_idx))
    }

    fn lower_hierarchy_instance(
        &mut self,
        instance: &ast::HierarchicalInstance,
        full_decl: Idx<Inst>,
    ) -> Option<HierarchicalInst> {
        let name = instance.name_of_instance()?;
        let ident = self.lower_ident(&name.identifier()?)?;
        let mut dimensions: SmallVec<[Dimension; 1]> = SmallVec::new();
        for dim in name.unpacked_dimensions() {
            let dim = self.lower_unpacked_dimension(&dim)?;
            dimensions.push(dim);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let list = instance.list_of_port_connections()?;
        let port_connects = try_match! {
            list.list_of_ordered_port_connections(), ordered => {
                let mut connects: SmallVec<[Option<ExprId>; 1]> = SmallVec::new();
                for connect in ordered.ordered_port_connections() {
                    connects.push(connect.expression().map(|expr| self.lower_expr(&expr)));
                }
                Some(PortConnects::Ordered(connects))
            },
            list.list_of_named_port_connections(), named => {
                let mut connects: SmallVec<[(Ident, Option<ExprId>); 1]> = SmallVec::new();
                for connect in named.named_port_connections() {
                    let ident = self.lower_ident(&connect.identifier()?)?;
                    let expr = connect.expression().map(|expr| self.lower_expr(&expr));
                    connects.push((ident, expr));
                }
                Some(PortConnects::Named(connects))
            },
            _ => None,
        };
        Some(HierarchicalInst { ident, dimensions, port_connects, full_decl })
    }

    fn lower_continuous_assign(&mut self, assign: &ast::ContinuousAssign) -> Option<ModuleItem> {
        let assign = try_match! {
            assign.list_of_net_assignments(), net_assigns => {
                let drive_strength = assign.drive_strength().and_then(|strenght| data::lower_drive_strength(&strenght));
                let delay = assign.delay3().and_then(|delay| self.lower_delay3(&delay));
                let mut assigns: SmallVec<[Assign; 1]> = SmallVec::new();
                for assign in net_assigns.net_assignments() {
                    let lhs = self.lower_net_lvalue(&assign.net_lvalue()?)?;
                    let rhs = self.lower_expr(&assign.expression()?);
                    let op = AssignOp::Assign;
                    assigns.push(Assign{lhs, rhs, op});
                }
                ContinuousAssignment::Net {
                    drive_strength,
                    delay,
                    assigns,
                }
            },
            assign.list_of_variable_assignments(), var_assigns => {
                let delay_control = assign.delay_control().and_then(|delay| self.lower_delay_control(&delay));
                let mut assigns: SmallVec<[Assign; 1]> = SmallVec::new();
                for assign in var_assigns.variable_assignments() {
                    let lhs = self.lower_var_lvalue(&assign.variable_lvalue()?)?;
                    let rhs = self.lower_expr(&assign.expression()?);
                    let op = AssignOp::Assign;
                    assigns.push(Assign{lhs, rhs, op});
                }
                ContinuousAssignment::Var {
                    delay_control,
                    assigns,
                }
            },
            _ => { return None; }
        };
        Some(ModuleItem::ContinuousAssignment(assign))
    }
}

impl LowerDelay for ModuleLowerCtx<'_> {}

impl LowerDelayControl for ModuleLowerCtx<'_> {}

impl LowerEventExpr for ModuleLowerCtx<'_> {
    fn arena_event_exprs(&mut self) -> &mut Arena<EventExpr> {
        &mut self.module_decl.data.event_exprs
    }

    fn src_map_event_expr(&mut self) -> &mut SourceMap<InFile<ptr::EventExpressionPtr>, EventExpr> {
        &mut self.module_src_map.event_expr
    }
}

impl LowerTimingControl for ModuleLowerCtx<'_> {}

impl LowerStmt for ModuleLowerCtx<'_> {
    fn arena_stmts(&mut self) -> &mut Arena<Stmt> {
        &mut self.module_decl.data.stmts
    }

    fn arena_blocks(&mut self) -> &mut Arena<BlockInfo> {
        &mut self.module_decl.data.block_infos
    }

    fn src_map_stmt(&mut self) -> &mut SourceMap<StmtSrc, Stmt> {
        &mut self.module_src_map.stmt
    }

    fn src_map_blocks(&mut self) -> &mut SourceMap<InFile<LocalBlockSrc>, BlockInfo> {
        &mut self.module_src_map.block
    }
}

impl LowerPackOrGenItemDecl for ModuleLowerCtx<'_> {}
