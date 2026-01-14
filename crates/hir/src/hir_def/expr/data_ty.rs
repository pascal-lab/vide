use itertools::Either;
use smallvec::SmallVec;
use syntax::{
    SyntaxToken, TokenKind,
    ast::{self, AstNode},
};

use super::{ExprId, LowerExprCtx, Selector};
use crate::{container::InContainer, hir_def::aggregate::StructId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataTy {
    Builtin(BuiltinDataTyId),
    Named(NamedDataTy),
    Struct(InContainer<StructId>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuiltinDataTyId(pub salsa::InternId);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum BuiltinDataTy {
    Int { kind: IntKind, signing: bool },
    Vector { kind: VecKind, signing: bool, dimensions: SmallVec<[Option<Dimension>; 2]> },
    Real(Real),
    String,
    Void,
}

impl Default for BuiltinDataTy {
    fn default() -> Self {
        BuiltinDataTy::Vector {
            kind: VecKind::default(),
            signing: false,
            dimensions: SmallVec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum IntKind {
    Byte,
    ShortInt,
    Int,
    LongInt,
    Integer,
    Time,
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum VecKind {
    Bit,
    #[default]
    Logic,
    Reg,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Real {
    Real,
    ShortReal,
    RealTime,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum Dimension {
    Range(ExprId, ExprId),
    Size(ExprId),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum NamedDataTy {
    Ident(ExprId),
    Field(ExprId),
}

impl LowerExprCtx<'_> {
    pub(crate) fn lower_data_ty(&mut self, ty: ast::DataType) -> DataTy {
        use ast::DataType::*;
        let ty = match ty {
            KeywordType(ty) => Either::Left(self.lower_keyword_ty(ty)),
            NamedType(named_type) => Either::Right(self.lower_named_ty(named_type)),
            IntegerType(ty) => Either::Left(self.lower_integer_type(ty)),
            ImplicitType(ty) => Either::Left(self.lower_implicit_type(ty)),
            EnumType(enum_ty) => Either::Right(self.lower_enum_type(enum_ty)),
            _ => unimplemented!("{:?}", ty.syntax().kind()),
        };
        match ty {
            Either::Left(ty) => DataTy::Builtin(self.db.intern_ty(ty)),
            Either::Right(ty) => DataTy::Named(ty),
        }
    }

    fn lower_keyword_ty(&mut self, ty: ast::KeywordType) -> BuiltinDataTy {
        use ast::KeywordType::*;
        match ty {
            StringType(_) => BuiltinDataTy::String,
            RealType(_) => BuiltinDataTy::Real(Real::Real),
            ShortRealType(_) => BuiltinDataTy::Real(Real::ShortReal),
            RealTimeType(_) => BuiltinDataTy::Real(Real::RealTime),
            VoidType(_) => BuiltinDataTy::Void,
            _ => unimplemented!("{:?}", ty.syntax().kind()),
        }
    }

    fn lower_named_ty(&mut self, ty: ast::NamedType) -> NamedDataTy {
        let expr_id = self.lower_expr(ast::Expression::cast(ty.name().syntax()).unwrap());

        use ast::Name::*;
        match ty.name() {
            IdentifierName(_) => NamedDataTy::Ident(expr_id),
            ScopedName(_) => NamedDataTy::Field(expr_id),
            _ => unreachable!("{:?}", ty.syntax().kind()),
        }
    }

    fn lower_enum_type(&mut self, _enum_ty: ast::EnumType) -> NamedDataTy {
        // For now, treat enum types as implicit types
        // TODO: properly handle enum member completion
        // We return a missing expression since enum types are anonymous
        let expr_id = self.alloc_missing();
        NamedDataTy::Ident(expr_id)
    }

    fn lower_integer_type(&mut self, ty: ast::IntegerType) -> BuiltinDataTy {
        use ast::IntegerType::*;
        let kind = match ty {
            TimeType(_) => Either::Left(IntKind::Time),
            ShortIntType(_) => Either::Left(IntKind::ShortInt),
            IntType(_) => Either::Left(IntKind::Int),
            IntegerType(_) => Either::Left(IntKind::Integer),
            LongIntType(_) => Either::Left(IntKind::LongInt),
            ByteType(_) => Either::Left(IntKind::Byte),
            RegType(_) => Either::Right(VecKind::Reg),
            BitType(_) => Either::Right(VecKind::Bit),
            LogicType(_) => Either::Right(VecKind::Logic),
        };

        let signing = Self::lower_signing(ty.signing())
            .unwrap_or(matches!(kind, Either::Left(IntKind::Time) | Either::Right(_)));

        let dimensions = ty.dimensions().children().map(|dim| self.lower_dimension(dim)).collect();
        match kind {
            Either::Left(kind) => BuiltinDataTy::Int { kind, signing },
            Either::Right(kind) => BuiltinDataTy::Vector { kind, signing, dimensions },
        }
    }

    fn lower_implicit_type(&mut self, ty: ast::ImplicitType) -> BuiltinDataTy {
        let signing = Self::lower_signing(ty.signing()).unwrap_or(false);
        let dimensions = ty.dimensions().children().map(|dim| self.lower_dimension(dim)).collect();
        // Default to be Logic, see SV spec 6.7.1
        BuiltinDataTy::Vector { kind: VecKind::Logic, signing, dimensions }
    }

    fn lower_signing(signing: Option<SyntaxToken>) -> Option<bool> {
        match signing?.kind() {
            TokenKind::SIGNED_KEYWORD => Some(true),
            TokenKind::UNSIGNED_KEYWORD => Some(false),
            TokenKind::UNKNOWN => None,
            _ => unreachable!(),
        }
    }

    pub(crate) fn lower_dimension(&mut self, dim: ast::VariableDimension) -> Option<Dimension> {
        use ast::DimensionSpecifier::*;
        match dim.specifier()? {
            RangeDimensionSpecifier(spec) => match self.lower_selector(spec.selector()) {
                Selector::Bit(idx) => Some(Dimension::Size(idx)),
                Selector::Range(left, right) => Some(Dimension::Range(left, right)),
                _ => unreachable!("{:?}", spec.syntax().kind()),
            },
            _ => unimplemented!("{:?}", dim.syntax().kind()),
        }
    }
}

impl DataTy {
    pub(crate) fn is_ast_missing(ty: ast::DataType) -> bool {
        match ty {
            ast::DataType::ImplicitType(ty) => {
                ty.signing().is_none()
                    && ty.dimensions().children().count() == 0
                    && ty.placeholder().is_none()
            }
            _ => false,
        }
    }
}
