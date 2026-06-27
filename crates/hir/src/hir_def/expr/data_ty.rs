use itertools::Either;
use smallvec::SmallVec;
use syntax::{
    SyntaxNode, SyntaxToken, TokenKind,
    ast::{self, AstNode},
};

use super::{Expr, ExprId, LowerExprCtx, Selector};
use crate::{
    container::InContainer,
    hir_def::{aggregate::StructId, lower_ident},
};

// slang exposes enum types directly as `DataType::EnumType`, while struct and
// union types share `DataType::StructUnionType` and are lowered by the owning
// declaration/typedef container into `aggregate::StructDef` with a
// `StructKind`. Unpacked dimensions carry SV array shape: `[]` is dynamic,
// `[$]`/`[$:N]` is a queue, and `[string]`/other builtin key types are
// associative. Plain `[expr]` stays a fixed-size unpacked dimension;
// typedef-key and wildcard associative arrays need scope-aware key lowering and
// are left for a later construct PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataTy {
    Builtin(BuiltinDataTyId),
    Named(NamedDataTy),
    Struct(InContainer<StructId>),
    Enum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuiltinDataTyId(pub salsa::InternId);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum BuiltinDataTy {
    Int { kind: IntKind, signing: bool },
    Vector { kind: VecKind, signing: bool, dimensions: SmallVec<[Option<Dimension>; 2]> },
    Real(Real),
    String,
    Event,
    Chandle,
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
    Queue(Option<ExprId>),
    Assoc(ExprId),
    Dynamic,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum NamedDataTy {
    Ident(ExprId),
    Field(ExprId),
}

impl LowerExprCtx<'_> {
    pub(crate) fn lower_data_ty(&mut self, ty: ast::DataType) -> DataTy {
        use ast::DataType::*;
        match ty {
            KeywordType(ty) => DataTy::Builtin(self.db.intern_ty(self.lower_keyword_ty(ty))),
            NamedType(named_type) => DataTy::Named(self.lower_named_ty(named_type)),
            IntegerType(ty) => DataTy::Builtin(self.db.intern_ty(self.lower_integer_type(ty))),
            ImplicitType(ty) => DataTy::Builtin(self.db.intern_ty(self.lower_implicit_type(ty))),
            EnumType(enum_ty) => self.lower_enum_type(enum_ty),
            StructUnionType(_) | TypeReference(_) | VirtualInterfaceType(_) => {
                self.default_data_ty()
            }
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
            EventType(_) => BuiltinDataTy::Event,
            CHandleType(_) => BuiltinDataTy::Chandle,
            _ => BuiltinDataTy::default(),
        }
    }

    fn lower_named_ty(&mut self, ty: ast::NamedType) -> NamedDataTy {
        let expr_id = ast::Expression::cast(ty.name().syntax())
            .map(|expr| self.lower_expr(expr))
            .unwrap_or_else(|| self.alloc_missing());

        use ast::Name::*;
        match ty.name() {
            IdentifierName(_) => NamedDataTy::Ident(expr_id),
            ScopedName(_) => NamedDataTy::Field(expr_id),
            _ => NamedDataTy::Ident(expr_id),
        }
    }

    fn lower_enum_type(&mut self, _enum_ty: ast::EnumType) -> DataTy {
        DataTy::Enum
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

        let signing = Self::lower_signing(ty.signing()).unwrap_or(matches!(kind, Either::Left(_)));

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

    pub(crate) fn lower_implicit_data_ty(&mut self, ty: ast::ImplicitType) -> DataTy {
        DataTy::Builtin(self.db.intern_ty(self.lower_implicit_type(ty)))
    }

    fn lower_signing(signing: Option<SyntaxToken>) -> Option<bool> {
        match signing?.kind() {
            TokenKind::SIGNED_KEYWORD => Some(true),
            TokenKind::UNSIGNED_KEYWORD => Some(false),
            TokenKind::UNKNOWN => None,
            _ => None,
        }
    }

    pub(crate) fn lower_dimension(&mut self, dim: ast::VariableDimension) -> Option<Dimension> {
        use ast::DimensionSpecifier::*;
        match dim.specifier() {
            None => Some(Dimension::Dynamic),
            Some(RangeDimensionSpecifier(spec)) => self.lower_range_dimension(spec),
            Some(QueueDimensionSpecifier(spec)) => Some(Dimension::Queue(
                spec.max_size_clause().map(|clause| self.lower_expr(clause.expr())),
            )),
            Some(WildcardDimensionSpecifier(_)) => None,
        }
    }

    fn lower_range_dimension(&mut self, spec: ast::RangeDimensionSpecifier) -> Option<Dimension> {
        let selector = spec.selector();
        if let ast::Selector::BitSelect(bit_select) = selector {
            let expr = bit_select.expr();
            if let Some(key) = Self::associative_dimension_key_token(expr) {
                let expr_id = lower_ident(Some(key))
                    .map(Expr::Ident)
                    .map(|expr| self.exprs.alloc(expr))
                    .unwrap_or_else(|| self.lower_expr(expr));
                return Some(Dimension::Assoc(expr_id));
            }
            Some(Dimension::Size(self.lower_expr(expr)))
        } else {
            match self.lower_selector(selector) {
                Selector::Range(left, right) => Some(Dimension::Range(left, right)),
                _ => None,
            }
        }
    }

    fn associative_dimension_key_token(expr: ast::Expression) -> Option<SyntaxToken> {
        let token = first_token(expr.syntax())?;
        is_builtin_dimension_key_token(token.kind()).then_some(token)
    }

    fn default_data_ty(&self) -> DataTy {
        DataTy::Builtin(self.db.intern_ty(BuiltinDataTy::default()))
    }
}

fn first_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    for idx in 0..node.child_count() {
        if let Some(token) = node.child_token(idx) {
            return Some(token);
        }
        if let Some(token) = node.child_node(idx).and_then(first_token) {
            return Some(token);
        }
    }
    None
}

fn is_builtin_dimension_key_token(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::STRING_KEYWORD
            | TokenKind::BYTE_KEYWORD
            | TokenKind::SHORT_INT_KEYWORD
            | TokenKind::INT_KEYWORD
            | TokenKind::LONG_INT_KEYWORD
            | TokenKind::INTEGER_KEYWORD
            | TokenKind::TIME_KEYWORD
            | TokenKind::BIT_KEYWORD
            | TokenKind::LOGIC_KEYWORD
            | TokenKind::REG_KEYWORD
    )
}

impl DataTy {
    pub(crate) fn is_ast_missing(ty: ast::DataType) -> bool {
        match ty {
            ast::DataType::ImplicitType(ty) => {
                ty.signing().is_none() && ty.dimensions().children().count() == 0
            }
            _ => false,
        }
    }
}
