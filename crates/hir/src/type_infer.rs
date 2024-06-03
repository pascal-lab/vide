use la_arena::Idx;
use rustc_hash::FxHashMap;

use crate::{
    container::{InContainer, InModule},
    db::HirDb,
    hir_def::{
        block::BlockId,
        data::{DataType, SubDecl},
        expr::ExprId,
        module::{module_item::ModuleInst, port::PortDecl},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldResolution {
    PortDecl(InContainer<Idx<PortDecl>>),
    SubDecl(InContainer<Idx<SubDecl>>),
    Block(BlockId),
    Inst(InModule<Idx<ModuleInst>>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TyInferResult {
    pub field2def_cache: FxHashMap<ExprId, FieldResolution>,
    pub expr2ty_cache: FxHashMap<ExprId, DataType>,
    pub diagnostics: Vec<TyInferDiagnostic>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TyInferDiagnostic {}

#[derive(Debug, Clone)]
pub(crate) struct TyInferCtx<'a> {
    pub(crate) db: &'a dyn HirDb,
    pub(crate) infer: TyInferResult,
    pub(crate) diagnostics: Vec<TyInferDiagnostic>,
}

impl<'a> TyInferCtx<'a> {
    pub fn new(db: &'a dyn HirDb) -> Self {
        TyInferCtx { db, infer: TyInferResult::default(), diagnostics: Vec::new() }
    }

    // pub fn infer_ty(&mut self, expr_id: ExprId) -> DataType {
    //
    // }
}
