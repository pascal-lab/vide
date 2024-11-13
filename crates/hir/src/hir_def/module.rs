use continuous_assgin::{ContAssign, ContAssignId, ContAssignSrc};
use instantiation::{
    Instance, InstanceId, InstanceSrc, Instantiation, InstantiationId, InstantiationSrc,
    ParamAssign, ParamAssignId, ParamAssignSrc, PortConn, PortConnId, PortConnSrc,
};
use la_arena::{Arena, Idx, IdxRange, RawIdx};
use port::{
    AnsiPort, AnsiPortId, AnsiPortSrc, NonAnsiPort, NonAnsiPortId, NonAnsiPortSrc, PortDecl,
    PortDeclId, PortDeclSrc, PortRef, PortRefId, PortRefSrc, PortSrcs, Ports,
};
use proc_macro_utils::define_container;
use syntax::ast::{self, AstNode, PortList};
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use super::{
    HirData, Ident,
    block::{BlockInfo, BlockSrc, LocalBlockId},
    declaration::{
        Declaration, DeclarationId, DeclarationSrc, LowerDeclaration, impl_lower_declaration,
    },
    expr::{
        Expr, ExprId, ExprSrc,
        declarator::{DeclId, Declarator, DeclaratorSrc, impl_lower_decl},
        impl_lower_expr,
        timing_control::{EventExpr, EventExprId, EventExprSrc, impl_lower_event_expr},
    },
    proc::{LowerProc, LowerProcCtx, Proc, ProcId, ProcSrc},
    stmt::{Stmt, StmtId, StmtSrc, impl_lower_stmt},
    ty::NetKind,
};
use crate::{
    container::InFile,
    db::{HirDb, InternDb},
    define_src_with_name,
    file::HirFileId,
    source_map::{SourceMap, ToAstNode},
};

pub mod continuous_assgin;
pub mod instantiation;
pub mod port;

define_container! {
    #[derive(Default, Debug, PartialEq, Eq, Clone)]
    pub struct Module | ModuleSourceMap {
        name: Option<Ident>,
        items: Arena<ModuleItem>,

        param_ports: Option<IdxRange<Declaration>>,
        ports | port_srcs: Ports[_ | PortSrcs] => {
            NonAnsiPort[NonAnsiPortId | NonAnsiPortSrc],
            AnsiPort[AnsiPortId | AnsiPortSrc],
            PortRef[PortRefId | PortRefSrc],
        },
        port_decls | prot_decl_srcs: PortDecl[PortDeclId | PortDeclSrc],

        cont_assigns | assign_srcs: ContAssign[ContAssignId | ContAssignSrc],
        declarations | declaration_srcs: Declaration[DeclarationId | DeclarationSrc],

        instantiations | instantiation_srcs: Instantiation[InstantiationId | InstantiationSrc],
        inst_param_assigns | inst_param_assign_srcs: ParamAssign[ParamAssignId | ParamAssignSrc],
        instances | instance_srcs: Instance[InstanceId | InstanceSrc],
        inst_port_conns | inst_port_conn_srcs: PortConn[PortConnId | PortConnSrc],

        procs | proc_srcs: Proc[ProcId | ProcSrc],

        exprs | expr_srcs: Expr[ExprId | ExprSrc],
        event_exprs | event_expr_srcs: EventExpr[EventExprId | EventExprSrc],
        decls | decl_srcs: Declarator[DeclId | DeclaratorSrc],
        stmts | stmt_srcs: Stmt[StmtId | StmtSrc] => {
            Stmt[StmtId | StmtSrc],
            BlockInfo[LocalBlockId | BlockSrc],
        }
    }
}

define_src_with_name!(ModuleSrc(ast::ModuleDeclaration));

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum ModuleItem {
        ContAssignId,
        DeclarationId,
        InstantiationId,
        ProcId,
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
    pub(crate) fn lower_module_decl(&mut self, decl: ast::ModuleDeclaration) {
        let header = decl.header();
        if let Some(param_ports) = header.parameters() {
            for decls in param_ports.declarations().children() {
                self.declaration_ctx().lower_param_decl_base(decls);
            }

            let beg = Idx::from_raw(RawIdx::from(0));
            let end = self.module.declarations.nxt_idx();
            if beg != end {
                self.module.param_ports = Some(IdxRange::new(beg..end));
            }
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
                ContinuousAssign(assign) => self.lower_continuous_assign(assign).into(),
                DataDeclaration(data_decl) => {
                    self.declaration_ctx().lower_data_decl(data_decl).into()
                }
                NetDeclaration(net_decl) => self.declaration_ctx().lower_net_decl(net_decl).into(),
                ParameterDeclarationStatement(param_decl) => {
                    self.declaration_ctx().lower_param_decl_base(param_decl.parameter()).into()
                }
                HierarchyInstantiation(instantiation) => {
                    self.lower_instantiation(instantiation).into()
                }
                FunctionDeclaration(_fn_decl) => todo!(),
                ProceduralBlock(proc) => self.proc_ctx().lower_proc(proc).into(),
                // Ports
                PortDeclaration(port) => {
                    self.lower_port_decl(port);
                    continue;
                }
                ExplicitAnsiPort(_) | ImplicitAnsiPort(_) => unreachable!(),
                _ => unimplemented!("unhandled module member: {:?}", member.syntax().kind()),
            };
            self.module.items.alloc(idx);
        }
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
    };
    lower_ctx.lower_module_decl(ast_module);

    module.shrink_to_fit();
    module_source_map.shrink_to_fit();
    (Arc::new(module), Arc::new(module_source_map))
}
