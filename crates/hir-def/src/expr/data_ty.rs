use itertools::Either;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, SyntaxNode, SyntaxToken, TokenKind,
    ast::{self, AstNode},
};
use triomphe::Arc;

use super::{Expr, ExprId, Selector};
use crate::{
    Ident,
    aggregate::{EnumId, StructId},
    ast_id_map::SourceAstId,
    container::OwnerRef,
    lower::{LoweringCtx, LoweringStore},
    lower_ident, lower_ident_opt,
};

// slang exposes enum types directly as `DataType::EnumType`, while struct and
// union types share `DataType::StructUnionType` and are lowered by the owning
// declaration/typedef container into `aggregate::StructDef` with a
// `StructKind`. Unpacked dimensions carry SV array shape: `[]` is dynamic,
// `[$]`/`[$:N]` is a queue, and `[string]`/other builtin key types are
// associative. Plain `[expr]` stays a fixed-size unpacked dimension;
// typedef-key and wildcard associative arrays need scope-aware key lowering and
// are left for a later construct PR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataTy {
    Builtin(BuiltinDataTyId),
    Named(TypeRef),
    Struct(OwnerRef<StructId>),
    Enum(OwnerRef<EnumId>),
    Unsupported(SyntaxKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuiltinDataTyId(Arc<BuiltinDataTy>);

impl BuiltinDataTyId {
    pub fn new(ty: BuiltinDataTy) -> Self {
        Self(Arc::new(ty))
    }

    pub fn get(&self) -> &BuiltinDataTy {
        &self.0
    }
}

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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Dimension {
    Range(ExprId, ExprId),
    Size(ExprId),
    Queue(Option<ExprId>),
    Assoc(ExprId),
    Wildcard,
    Dynamic,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum TypePathKind {
    Unqualified,
    Package,
    Hierarchical,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum TypePathRecovery {
    Selectors,
    UnsupportedName(SyntaxKind),
    MissingIdentifier,
    MixedSeparators,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct TypeRef {
    segments: SmallVec<[Ident; 2]>,
    segment_sources: SmallVec<[SourceAstId; 2]>,
    path_kind: TypePathKind,
    source: SourceAstId,
    recovery: Option<TypePathRecovery>,
}

impl TypeRef {
    fn new(
        segments: SmallVec<[Ident; 2]>,
        segment_sources: SmallVec<[SourceAstId; 2]>,
        path_kind: TypePathKind,
        source: SourceAstId,
        recovery: Option<TypePathRecovery>,
    ) -> Self {
        Self { segments, segment_sources, path_kind, source, recovery }
    }

    pub fn segments(&self) -> &[Ident] {
        &self.segments
    }

    pub fn segment_sources(&self) -> &[SourceAstId] {
        &self.segment_sources
    }

    pub fn path_kind(&self) -> TypePathKind {
        self.path_kind
    }

    pub fn source(&self) -> SourceAstId {
        self.source
    }

    pub fn recovery(&self) -> Option<TypePathRecovery> {
        self.recovery
    }

    pub fn is_valid(&self) -> bool {
        self.recovery.is_none()
    }
}

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn lower_data_ty(&mut self, ty: ast::DataType) -> DataTy {
        use ast::DataType::*;
        match ty {
            KeywordType(ty) => match self.lower_keyword_ty(ty) {
                Ok(ty) => DataTy::Builtin(BuiltinDataTyId::new(ty)),
                Err(kind) => DataTy::Unsupported(kind),
            },
            NamedType(named_type) => DataTy::Named(self.lower_named_ty(named_type)),
            IntegerType(ty) => DataTy::Builtin(BuiltinDataTyId::new(self.lower_integer_type(ty))),
            ImplicitType(ty) => DataTy::Builtin(BuiltinDataTyId::new(self.lower_implicit_type(ty))),
            EnumType(enum_ty) => self.lower_enum_type(enum_ty),
            StructUnionType(struct_ty) => {
                let container = self.current_owner();
                let struct_id = self.lower_body_struct_type(struct_ty);
                DataTy::Struct(OwnerRef::new(container, struct_id))
            }
            unsupported @ (TypeReference(_) | VirtualInterfaceType(_)) => {
                let kind = unsupported.syntax().kind();
                self.report_unsupported(unsupported.syntax(), "unsupported data type");
                DataTy::Unsupported(kind)
            }
        }
    }

    fn lower_keyword_ty(&mut self, ty: ast::KeywordType) -> Result<BuiltinDataTy, SyntaxKind> {
        use ast::KeywordType::*;
        match ty {
            StringType(_) => Ok(BuiltinDataTy::String),
            RealType(_) => Ok(BuiltinDataTy::Real(Real::Real)),
            ShortRealType(_) => Ok(BuiltinDataTy::Real(Real::ShortReal)),
            RealTimeType(_) => Ok(BuiltinDataTy::Real(Real::RealTime)),
            VoidType(_) => Ok(BuiltinDataTy::Void),
            EventType(_) => Ok(BuiltinDataTy::Event),
            CHandleType(_) => Ok(BuiltinDataTy::Chandle),
            unsupported @ (PropertyType(_) | Untyped(_) | SequenceType(_)) => {
                let kind = unsupported.syntax().kind();
                self.report_unsupported(unsupported.syntax(), "unsupported keyword data type");
                Err(kind)
            }
        }
    }

    fn lower_named_ty(&mut self, ty: ast::NamedType) -> TypeRef {
        let name = ty.name();
        let source = self.source_id(name.syntax());
        let mut segments = SmallVec::new();
        let mut segment_sources = SmallVec::new();
        let mut separator = None;
        let mut recovery = None;
        self.lower_name_segments(
            name,
            &mut segments,
            &mut segment_sources,
            &mut separator,
            &mut recovery,
        );

        if let Some(kind) = recovery {
            segments.clear();
            segment_sources.clear();
            let message = match kind {
                TypePathRecovery::Selectors => "type path selectors are unsupported",
                TypePathRecovery::UnsupportedName(_) => "unsupported type path name",
                TypePathRecovery::MissingIdentifier => "type path is missing an identifier",
                TypePathRecovery::MixedSeparators => {
                    "type path mixes package and hierarchical separators"
                }
            };
            self.report_unsupported(name.syntax(), message);
        }

        TypeRef::new(
            segments,
            segment_sources,
            separator.unwrap_or(TypePathKind::Unqualified),
            source,
            recovery,
        )
    }

    fn lower_name_segments(
        &mut self,
        name: ast::Name,
        segments: &mut SmallVec<[Ident; 2]>,
        segment_sources: &mut SmallVec<[SourceAstId; 2]>,
        separator: &mut Option<TypePathKind>,
        recovery: &mut Option<TypePathRecovery>,
    ) {
        let mut set_recovery = |kind| {
            if recovery.is_none() {
                *recovery = Some(kind);
            }
        };

        match name {
            ast::Name::IdentifierName(name) => {
                let source = self.source_id(name.syntax());
                let Some(ident) = lower_ident_opt(name.identifier()) else {
                    set_recovery(TypePathRecovery::MissingIdentifier);
                    return;
                };
                segments.push(ident);
                segment_sources.push(source);
            }
            ast::Name::IdentifierSelectName(name) => {
                if name.selectors().children().next().is_some() {
                    set_recovery(TypePathRecovery::Selectors);
                    return;
                }
                let source = self.source_id(name.syntax());
                let Some(ident) = lower_ident_opt(name.identifier()) else {
                    set_recovery(TypePathRecovery::MissingIdentifier);
                    return;
                };
                segments.push(ident);
                segment_sources.push(source);
            }
            ast::Name::ScopedName(name) => {
                let next_separator = match name.separator().map(|token| token.kind()) {
                    Some(TokenKind::DOUBLE_COLON) => TypePathKind::Package,
                    Some(TokenKind::DOT) => TypePathKind::Hierarchical,
                    Some(_) | None => {
                        set_recovery(TypePathRecovery::UnsupportedName(name.syntax().kind()));
                        return;
                    }
                };
                if let Some(previous) = *separator {
                    if previous != next_separator {
                        set_recovery(TypePathRecovery::MixedSeparators);
                    }
                } else {
                    *separator = Some(next_separator);
                }
                self.lower_name_segments(
                    name.left(),
                    segments,
                    segment_sources,
                    separator,
                    recovery,
                );
                self.lower_name_segments(
                    name.right(),
                    segments,
                    segment_sources,
                    separator,
                    recovery,
                );
            }
            _ => set_recovery(TypePathRecovery::UnsupportedName(name.syntax().kind())),
        }
    }

    fn lower_enum_type(&mut self, enum_ty: ast::EnumType) -> DataTy {
        let container = self.current_owner();
        let enum_id = self.lower_body_enum_type(enum_ty);
        DataTy::Enum(OwnerRef::new(container, enum_id))
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
        DataTy::Builtin(BuiltinDataTyId::new(self.lower_implicit_type(ty)))
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
            Some(WildcardDimensionSpecifier(_)) => Some(Dimension::Wildcard),
        }
    }

    fn lower_range_dimension(&mut self, spec: ast::RangeDimensionSpecifier) -> Option<Dimension> {
        let selector = spec.selector();
        if let ast::Selector::BitSelect(bit_select) = selector {
            let expr = bit_select.expr();
            if let Some(key) = Self::associative_dimension_key_token(expr) {
                let expr_id = lower_ident(Some(key))
                    .map(Expr::Ident)
                    .map(|expr| self.expressions().0.alloc(expr))
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
