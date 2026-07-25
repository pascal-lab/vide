use hir_def::{
    Ident,
    container::{InContainer, SubroutineScope},
    def_id::DefId,
    expr::ExprId,
    subroutine::SubroutineKind,
    symbol::Resolution,
    typedef::TypedefId,
};

use crate::{
    db::TyDb,
    display::{HirDisplay, HirDisplayError},
    infer::{members_of_ty, normalize_data_ty, packed_bit_width, type_class},
    ty::{Ty, TyClass, TyResult},
};

/// A diagnostic produced while determining a semantic type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDiagnostic {
    TypedefCycle(InContainer<TypedefId>),
}

/// Semantic type information returned by the type system.
///
/// The representation and salsa query result stay private so callers do not
/// depend on inference internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type(TyResult);

impl Type {
    pub fn unknown() -> Self {
        Self(TyResult::new(Ty::Unknown))
    }

    pub fn diagnostics(&self) -> &[TypeDiagnostic] {
        &self.0.diagnostics
    }

    pub(crate) fn ty(&self) -> &Ty {
        &self.0.ty
    }
}

impl From<TyResult> for Type {
    fn from(result: TyResult) -> Self {
        Self(result)
    }
}

/// A named member and its semantic type.
#[derive(Debug, Clone)]
pub struct Member {
    name: Ident,
    ty: Type,
}

impl Member {
    pub fn name(&self) -> &Ident {
        &self.name
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }

    pub fn into_name(self) -> Ident {
        self.name
    }
}

/// Result of comparing two known semantic value types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Compatible,
    Incompatible,
    Unknown,
}

/// Stable interface to semantic typing.
///
/// Salsa queries, HIR arena access, normalization, and type representation are
/// implementation details behind this interface.
#[derive(Clone, Copy)]
pub struct TypeSystem<'db> {
    db: &'db dyn TyDb,
}

impl<'db> TypeSystem<'db> {
    pub fn new(db: &'db dyn TyDb) -> Self {
        Self { db }
    }

    pub fn type_of_expr(&self, expr: InContainer<ExprId>) -> Type {
        self.db.infer_expr(expr).as_ref().clone()
    }

    pub fn type_of_resolution(&self, resolution: Resolution<DefId>) -> Type {
        self.db.infer_path_resolution(resolution).as_ref().clone()
    }

    pub fn type_of_def(&self, def: DefId) -> Type {
        self.type_of_resolution(Resolution::Unique(def))
    }

    pub fn type_of_subroutine_return(&self, subroutine: SubroutineScope) -> Type {
        match self.db.subroutine(subroutine).kind {
            SubroutineKind::Function { return_ty: Some(return_ty) } => {
                normalize_data_ty(self.db, subroutine.into(), return_ty).into()
            }
            SubroutineKind::Function { return_ty: None } | SubroutineKind::Task => Type::unknown(),
        }
    }

    pub fn members(&self, ty: &Type) -> Vec<Member> {
        members_of_ty(self.db, ty.ty())
            .into_iter()
            .map(|member| Member { name: member.name, ty: TyResult::new(member.ty).into() })
            .collect()
    }

    pub fn compatibility(&self, expected: &Type, candidate: &Type) -> Compatibility {
        let (Some(expected_class), Some(candidate_class)) =
            (type_class(self.db, expected.ty()), type_class(self.db, candidate.ty()))
        else {
            return Compatibility::Unknown;
        };
        if expected_class != candidate_class {
            return Compatibility::Incompatible;
        }
        if expected_class != TyClass::Integral {
            return Compatibility::Compatible;
        }

        match (packed_bit_width(self.db, expected.ty()), packed_bit_width(self.db, candidate.ty()))
        {
            (Some(expected), Some(candidate)) if expected == candidate => Compatibility::Compatible,
            (Some(_), Some(_)) => Compatibility::Incompatible,
            _ => Compatibility::Unknown,
        }
    }

    pub fn is_typed_value(&self, ty: &Type) -> bool {
        type_class(self.db, ty.ty()).is_some()
    }

    pub fn display_source(&self, ty: &Type) -> Result<String, HirDisplayError> {
        ty.ty().display_source(self.db)
    }

    pub fn display_declaration(&self, ty: &Type) -> Result<Option<String>, HirDisplayError> {
        match ty.ty() {
            Ty::Unknown
            | Ty::Error
            | Ty::Void
            | Ty::Module(_)
            | Ty::GenerateBlock(_)
            | Ty::Block(_) => Ok(None),
            _ => self.display_source(ty).map(Some),
        }
    }
}
