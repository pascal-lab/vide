use hir_def::{
    Ident,
    container::OwnerRef,
    def_id::DefId,
    expr::{ExprId, data_ty::TypePathRecovery},
    owner::OwnerId,
    subroutine::SubroutineKind,
    symbol::Resolution,
    typedef::TypedefId,
};
use syntax::SyntaxKind;
use triomphe::Arc;

use crate::{
    compatibility::{compatibility, is_typed_value},
    db::TyDb,
    display::{HirDisplay, HirDisplayError},
    members::members_of_ty,
    ty::{Ty, TyResult},
};

/// A diagnostic produced while determining a semantic type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDiagnostic {
    TypedefCycle(OwnerRef<TypedefId>),
    InvalidTypePath(TypePathRecovery),
    UnsupportedDataType(SyntaxKind),
}

/// Semantic type information returned by the type system.
///
/// The representation and salsa query result stay private so callers do not
/// depend on inference internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type(Arc<TyResult>);

impl Type {
    pub fn unknown() -> Self {
        Self(Arc::new(TyResult::new(Ty::Unknown)))
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
        Self(Arc::new(result))
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
#[derive(Clone)]
pub struct TypeSystem<'db> {
    db: &'db dyn TyDb,
    context: triomphe::Arc<hir_def::pathres::ResolutionContext>,
}

impl<'db> TypeSystem<'db> {
    pub fn new(
        db: &'db dyn TyDb,
        context: triomphe::Arc<hir_def::pathres::ResolutionContext>,
    ) -> Self {
        Self { db, context }
    }

    pub fn type_of_expr(&self, expr: OwnerRef<ExprId>) -> Type {
        crate::infer::type_of_expr_impl(self.db, &self.context, expr).into()
    }

    pub fn type_of_resolution(&self, resolution: Resolution<DefId>) -> Type {
        crate::infer::type_of_path_resolution_impl(self.db, &self.context, resolution).into()
    }

    pub fn type_of_def(&self, def: DefId) -> Type {
        self.type_of_resolution(Resolution::Unique(def))
    }

    pub fn type_of_subroutine_return(&self, subroutine: OwnerId) -> Type {
        match &self.db.subroutine(subroutine).kind {
            SubroutineKind::Function { return_ty: Some(return_ty) } => {
                crate::infer::normalize_data_ty(
                    self.db,
                    &self.context,
                    subroutine,
                    return_ty.clone(),
                )
                .into()
            }
            SubroutineKind::Function { return_ty: None } | SubroutineKind::Task => Type::unknown(),
        }
    }

    pub fn members(&self, ty: &Type) -> Vec<Member> {
        members_of_ty(self.db, &self.context, ty.ty())
            .into_iter()
            .map(|member| Member { name: member.name, ty: TyResult::new(member.ty).into() })
            .collect()
    }

    pub fn compatibility(&self, expected: &Type, candidate: &Type) -> Compatibility {
        compatibility(self.db, expected.ty(), candidate.ty())
    }

    pub fn is_typed_value(&self, ty: &Type) -> bool {
        is_typed_value(self.db, ty.ty())
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
