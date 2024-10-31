use la_arena::Arena;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};
use triomphe::Arc;
use utils::define_enum_deriving_from;

use super::{
    block::{BlockInfo, BlockSrc, LocalBlockId},
    declaration::{Declaration, DeclarationId, DeclarationSrc, LowerDeclaration},
    expr::{
        Expr, ExprId, ExprSrc,
        declarator::{DeclId, Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprId, EventExprSrc},
    },
    lower_ident,
    module::{LocalModuleId, ModuleInfo, ModuleSrc},
    proc::{LowerProc, LowerProcCtx, Proc, ProcId, ProcSrc},
    stmt::{Stmt, StmtId, StmtSrc},
};
use crate::{
    alloc_idx_and_src,
    db::{HirDb, InternDb},
    file::HirFileId,
    impl_arena_idx, impl_lower_decl, impl_lower_declaration, impl_lower_event_expr,
    impl_lower_expr, impl_lower_stmt, impl_source_map_idx,
    source_map::SourceMap,
};

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct HirFile {
    // Represente the item in order
    pub items: SmallVec<[FileItem; 3]>,

    // TODO: DataDecl, InterfaceDecl
    pub modules: Arena<ModuleInfo>,
    pub procs: Arena<Proc>,
    pub declarations: Arena<Declaration>,

    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
    pub stmts: Arena<Stmt>,
}

impl_arena_idx! { HirFile =>
    modules[ModuleInfo],
    procs[Proc],
    declarations[Declaration],

    exprs[Expr],
    event_exprs[EventExpr],
    decls[Declarator],
    stmts[Stmt],
    stmts[LocalBlockId => BlockInfo],
}

impl HirFile {
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
        self.modules.shrink_to_fit();
        self.procs.shrink_to_fit();
        self.declarations.shrink_to_fit();
        self.exprs.shrink_to_fit();
        self.event_exprs.shrink_to_fit();
        self.decls.shrink_to_fit();
        self.stmts.shrink_to_fit();
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum FileItem {
        LocalModuleId,
        ProcId,
        DeclarationId,
    }
}

// Definition for HirFileSourceMap
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct FileSourceMap {
    pub module_srcs: SourceMap<ModuleSrc, ModuleInfo>,
    pub proc_srcs: SourceMap<ProcSrc, Proc>,
    pub declaration_srcs: SourceMap<DeclarationSrc, Declaration>,

    pub expr_srcs: SourceMap<ExprSrc, Expr>,
    pub event_expr_srcs: SourceMap<EventExprSrc, EventExpr>,
    pub decl_srcs: SourceMap<DeclaratorSrc, Declarator>,
    pub stmt_srcs: SourceMap<StmtSrc, Stmt>,
}

impl_source_map_idx! { FileSourceMap =>
    module_srcs[ModuleSrc, LocalModuleId],
    proc_srcs[ProcSrc, ProcId],
    declaration_srcs[DeclarationSrc, DeclarationId],
    expr_srcs[ExprSrc, ExprId],
    event_expr_srcs[EventExprSrc, EventExprId],
    decl_srcs[DeclaratorSrc, DeclId],
    stmt_srcs[StmtSrc, StmtId],
    stmt_srcs[BlockSrc, LocalBlockId],
}

impl FileSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.module_srcs.shrink_to_fit();
        self.proc_srcs.shrink_to_fit();
        self.declaration_srcs.shrink_to_fit();

        self.expr_srcs.shrink_to_fit();
        self.event_expr_srcs.shrink_to_fit();
        self.decl_srcs.shrink_to_fit();
        self.stmt_srcs.shrink_to_fit();
    }
}

pub(crate) struct LowerFileCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,

    pub(crate) file: &'a mut HirFile,
    pub(crate) file_source_map: &'a mut FileSourceMap,
}

impl_lower_expr!(LowerFileCtx<'_>, file, file_source_map);
impl_lower_decl!(LowerFileCtx<'_>, file, file_source_map);
impl_lower_event_expr!(LowerFileCtx<'_>, file, file_source_map);
impl_lower_stmt!(LowerFileCtx<'_>, file_id, file, file_source_map);
impl_lower_declaration!(LowerFileCtx<'_>, file, file_source_map);

impl LowerProc for LowerFileCtx<'_> {
    fn proc_ctx(&mut self) -> LowerProcCtx<'_> {
        LowerProcCtx {
            db: self.db,
            file_id: self.file_id,
            cont_id: self.file_id.into(),
            procs: &mut self.file.procs,
            proc_srcs: &mut self.file_source_map.proc_srcs,

            stmts: &mut self.file.stmts,
            stmt_srcs: &mut self.file_source_map.stmt_srcs,

            exprs: &mut self.file.exprs,
            expr_srcs: &mut self.file_source_map.expr_srcs,

            event_exprs: &mut self.file.event_exprs,
            event_expr_srcs: &mut self.file_source_map.event_expr_srcs,

            decls: &mut self.file.decls,
            decl_srcs: &mut self.file_source_map.decl_srcs,
        }
    }
}

impl LowerFileCtx<'_> {
    pub(crate) fn lower_file(&mut self, root: ast::CompilationUnit) {
        for member in root.members().children() {
            use ast::Member::*;
            let idx = match member {
                ModuleDeclaration(decl) => {
                    let name = lower_ident(decl.header().name());

                    alloc_idx_and_src! {
                        ModuleInfo { name } => self.file.modules,
                        decl => self.file_source_map.module_srcs,
                    }
                    .into()
                }
                ProceduralBlock(proc) => self.proc_ctx().lower_proc(proc).into(),
                DataDeclaration(data_decl) => {
                    self.declaration_ctx().lower_data_decl(data_decl).into()
                }
                NetDeclaration(net_decl) => self.declaration_ctx().lower_net_decl(net_decl).into(),
                _ => unimplemented!(),
            };
            self.file.items.push(idx);
        }
    }
}

pub(crate) fn hir_file_with_source_map_query(
    db: &dyn HirDb,
    file_id: HirFileId,
) -> (Arc<HirFile>, Arc<FileSourceMap>) {
    let mut hir_file = HirFile::default();
    let mut source_map = FileSourceMap::default();

    let tree = db.parse(file_id);
    let Some(root) = tree.root().and_then(ast::CompilationUnit::cast) else {
        return (Arc::new(hir_file), Arc::new(source_map));
    };

    let mut lower_ctx =
        LowerFileCtx { db, file_id, file: &mut hir_file, file_source_map: &mut source_map };
    lower_ctx.lower_file(root);

    hir_file.shrink_to_fit();
    source_map.shrink_to_fit();

    (Arc::new(hir_file), Arc::new(source_map))
}
