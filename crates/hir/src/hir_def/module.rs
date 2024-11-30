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
    block::{BlockInfo, BlockSrc, LocalBlockId},
    declaration::{
        Declaration, DeclarationId, DeclarationSrc, LowerDeclaration, impl_lower_declaration,
    },
    expr::{
        Expr, ExprSrc,
        declarator::{Declarator, DeclaratorSrc, impl_lower_decl},
        impl_lower_expr,
        timing_control::{EventExpr, EventExprSrc, impl_lower_event_expr},
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

        param_ports: Option<IdxRange<Declaration>>,
        ports: Ports => {
            [NonAnsiPortId | NonAnsiPort],
            [PortRefId | PortRef],
            [PortDeclId | PortDecl],
        },

        cont_assigns: [ContAssign],
        declarations: [Declaration],

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
        doc_tree: RegionTree,

        port_srcs: PortSrcs => {
            [NonAnsiPortId | NonAnsiPortSrc],
            [PortRefId | PortRefSrc],
            [PortDeclId | PortDeclSrc],
        },

        assign_srcs: [ContAssign | ContAssignSrc],
        declaration_srcs: [Declaration | DeclarationSrc],

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

impl ModuleSourceMap {
    pub fn item_to_ptr(&self, item: &ModuleItem) -> SyntaxNodePtr {
        match item {
            ModuleItem::ContAssignId(idx) => self.get(*idx).0,
            ModuleItem::DeclarationId(idx) => self.get(*idx).ptr(),
            ModuleItem::InstantiationId(idx) => self.get(*idx).0,
            ModuleItem::ProcId(idx) => self.get(*idx).0,
            ModuleItem::PortDeclId(idx) => self.get(*idx).ptr(),
        }
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum ModuleItem {
        ContAssignId,
        DeclarationId,
        InstantiationId,
        ProcId,
        PortDeclId,
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
    pub(crate) doc_tree: RegionTreeBuilder,
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
                self.doc_tree.handle_node(decls.syntax());
            }

            let beg = Idx::from_raw(RawIdx::from(0));
            let end = self.module.declarations.nxt_idx();
            if beg != end {
                self.module.param_ports = Some(IdxRange::new(beg..end));
            }

            self.doc_tree.stage(param_ports.close_paren());
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
                PortDeclaration(port) => self.lower_port_decl(port).into(),
                ExplicitAnsiPort(_) | ImplicitAnsiPort(_) => unreachable!(),
                EmptyMember(_) => continue,
                _ => unimplemented!("unhandled module member: {:?}", member.syntax().kind()),
            };
            self.module_source_map.items.push(idx);
            self.doc_tree.handle_node(member.syntax());
        }
        self.doc_tree.stage(decl.endmodule());

        self.doc_tree.handle_tok(decl.endmodule());
        self.module_source_map.doc_tree = self.doc_tree.finish();
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
        doc_tree: RegionTreeBuilder::new(),
    };
    lower_ctx.lower_module_decl(ast_module);

    module.shrink_to_fit();
    module_source_map.shrink_to_fit();
    (Arc::new(module), Arc::new(module_source_map))
}
