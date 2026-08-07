use hir_def::{
    Ident,
    aggregate::StructId,
    container::{ArenaOwnerId, InContainer},
    def_id::DefId,
    expr::{ExprId, data_ty::BuiltinDataTyId},
    module::{ModuleId, generate::GenerateBlockId},
    owner::OwnerId,
    typedef::TypedefId,
};

use crate::TypeDiagnostic;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinTy {
    Data { id: BuiltinDataTyId, container: ArenaOwnerId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Ty {
    Unknown,
    Error,
    Void,
    Builtin(BuiltinTy),
    Struct(InContainer<StructId>),
    Enum(DefId),
    Union(DefId),
    Queue { elem: Box<Ty>, size: Option<ExprId> },
    Assoc { key: Box<Ty>, elem: Box<Ty> },
    Dynamic(Box<Ty>),
    Event,
    Chandle,
    Alias { typedef: InContainer<TypedefId>, target: Box<Ty> },
    Module(ModuleId),
    Checker(DefId),
    Covergroup(DefId),
    VirtualInterface { def: DefId, modport: Option<DefId> },
    GenerateBlock(GenerateBlockId),
    Block(OwnerId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TyResult {
    pub(crate) ty: Ty,
    pub(crate) diagnostics: Vec<TypeDiagnostic>,
}

impl TyResult {
    pub(crate) fn new(ty: Ty) -> Self {
        TyResult { ty, diagnostics: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TyMember {
    pub(crate) name: Ident,
    pub(crate) ty: Ty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TyClass {
    Integral,
    Real,
    String,
}
