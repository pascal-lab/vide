use la_arena::Arena;
use proc_macro_utils::define_container;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};
use triomphe::Arc;
use utils::define_enum_deriving_from;

use super::{
    alloc_idx_and_src,
    block::{BlockSrc, LocalBlockId, BlockInfo},
    declaration::{
        Declaration, DeclarationId, DeclarationSrc, LowerDeclaration, impl_lower_declaration,
    },
    expr::{
        Expr, ExprSrc,
        declarator::{Declarator, DeclaratorSrc, impl_lower_decl},
        impl_lower_expr,
        timing_control::{EventExpr, EventExprSrc, impl_lower_event_expr},
    },
    module::{LocalModuleId, ModuleInfo, ModuleSrc},
    proc::{LowerProc, LowerProcCtx, Proc, ProcId, ProcSrc},
    stmt::{Stmt, StmtId, StmtSrc, impl_lower_stmt},
};
use crate::{
    db::{HirDb, InternDb},
    file::HirFileId,
    hir_def::lower_ident_opt,
    source_map::SourceMap,
};

define_container! {
    #[derive(Default, Debug, PartialEq, Eq, Clone)]
    pub struct HirFile {
        items: SmallVec<[FileItem; 3]>,

        modules: [ModuleInfo],
        procs: [Proc],

        declarations: [Declaration],
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
    #[derive(Default, Debug, PartialEq, Eq, Clone)]
    pub struct FileSourceMap {
        module_srcs: [ModuleInfo | ModuleSrc],
        proc_srcs: [Proc | ProcSrc],

        declaration_srcs: [Declaration | DeclarationSrc],
        expr_srcs: [Expr | ExprSrc],
        event_expr_srcs: [EventExpr | EventExprSrc],
        decl_srcs: [Declarator | DeclaratorSrc],
        stmt_srcs: [Stmt | StmtSrc] => {
            [StmtId | StmtSrc],
            [LocalBlockId | BlockSrc],
        }
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
                    let name = lower_ident_opt(decl.header().name());

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
