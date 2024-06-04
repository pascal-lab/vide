mod lower;
pub mod module_item;
pub mod port;

use std::ops::Index;

use itertools::Either;
use la_arena::{Arena, Idx, IdxRange};
use syntax::ast::ptr;
use triomphe::Arc;
use utils::try_;

use super::{
    block::{block_src::BlockSrc, BlockInfo},
    ModuleId,
};
use crate::{
    container::InFile,
    db::HirDb,
    hir_def::{
        control::EventExpr,
        data::{DataDecl, DataDeclSrc, SubDecl, SubDeclSrc},
        expr::{Expr, ExprSrc},
        //tf::TFDecl,
        impl_arena_idx,
        module::{
            module_item::{HierarchicalInst, ModuleInst, ModuleItem, ModuleItemSrc},
            port::{Port, PortDecl},
        },
        stmt::{Stmt, StmtSrc},
        Ident,
    },
    impl_source_map_idx,
    source_map::SourceMap,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Module {
    pub ident: Ident,
    pub param_port_list: Option<IdxRange<DataDecl>>,
    pub ports: Arena<Port>,
    pub module_items: Arena<ModuleItem>,
    pub data: ModuleData,
}

impl_arena_idx!(Module for
    ports[Port],
    module_items[ModuleItem],
    data[Expr],
    data[EventExpr],
    data[SubDecl],
    data[DataDecl],
    data[PortDecl],
    data[Stmt],
    data[BlockInfo],
    data[HierarchicalInst],
    data[ModuleInst],
);

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct ModuleData {
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub sub_decls: Arena<SubDecl>,
    pub data_decls: Arena<DataDecl>,
    pub port_decls: Arena<PortDecl>,
    pub stmts: Arena<Stmt>,
    pub block_infos: Arena<BlockInfo>,
    pub hierarchical_insts: Arena<HierarchicalInst>,
    pub insts: Arena<ModuleInst>,
}

impl_arena_idx!(ModuleData for
    exprs[Expr],
    event_exprs[EventExpr],
    sub_decls[SubDecl],
    data_decls[DataDecl],
    port_decls[PortDecl],
    stmts[Stmt],
    block_infos[BlockInfo],
    hierarchical_insts[HierarchicalInst],
    insts[ModuleInst],
);

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct ModuleSourceMap {
    pub port_decl:
        SourceMap<Either<InFile<ptr::PortPtr>, InFile<ptr::AnsiPortDeclarationPtr>>, Port>,
    pub module_item: SourceMap<ModuleItemSrc, ModuleItem>,
    pub expr: SourceMap<ExprSrc, Expr>,
    pub event_expr: SourceMap<InFile<ptr::EventExpressionPtr>, EventExpr>,
    pub sub_decl: SourceMap<SubDeclSrc, SubDecl>,
    pub data_decl: SourceMap<DataDeclSrc, DataDecl>,
    pub port_def: SourceMap<
        Either<InFile<ptr::PortDeclarationPtr>, InFile<ptr::AnsiPortDeclarationPtr>>,
        PortDecl,
    >,
    pub stmt: SourceMap<StmtSrc, Stmt>,
    pub block: SourceMap<BlockSrc, BlockInfo>,
    pub hierarchy_inst: SourceMap<InFile<ptr::HierarchicalInstancePtr>, HierarchicalInst>,
    pub inst: SourceMap<InFile<ptr::InstantiationPtr>, ModuleInst>,
}

impl_source_map_idx! { ModuleSourceMap for
    port_decl[Either<InFile<ptr::PortPtr>, InFile<ptr::AnsiPortDeclarationPtr>>, Port],
    module_item[ModuleItemSrc, ModuleItem],
    expr[ExprSrc, Expr],
    event_expr[InFile<ptr::EventExpressionPtr>, EventExpr],
    sub_decl[SubDeclSrc, SubDecl],
    data_decl[DataDeclSrc, DataDecl],
    port_def[Either<InFile<ptr::PortDeclarationPtr>, InFile<ptr::AnsiPortDeclarationPtr>>, PortDecl],
    stmt[StmtSrc, Stmt],
    block[BlockSrc, BlockInfo],
    hierarchy_inst[InFile<ptr::HierarchicalInstancePtr>, HierarchicalInst],
    inst[InFile<ptr::InstantiationPtr>, ModuleInst],
}

pub(crate) fn module_with_source_map_query(
    db: &dyn HirDb,
    module_id: ModuleId,
) -> (Arc<Module>, Arc<ModuleSourceMap>) {
    let (hir_file, file_source_map) = db.hir_file_with_source_map(module_id.file_id);
    let ident = hir_file.data[module_id.value].ident.clone();
    let mut module_decl = Module {
        ident,
        param_port_list: None,
        ports: Arena::default(),
        module_items: Arena::default(),
        data: ModuleData::default(),
    };
    let mut module_src_map = ModuleSourceMap::default();

    try_! {
        let module_ptr = file_source_map.modules.get_src(module_id.value)?;
        let tree = db.hir_syntax_tree(module_id.file_id)?;
        let module_node = module_ptr.value.to_node(tree.tree())?;
        let file_text = db.hir_file_text(module_id.file_id);
        let mut ctx = lower::ModuleLowerCtx {
            db,
            module_id,
            module_decl: &mut module_decl,
            module_src_map: &mut module_src_map,
            file_text: file_text.as_ref(),
        };
        ctx.lower_module_decl(&module_node);
    };

    (Arc::new(module_decl), Arc::new(module_src_map))
}
