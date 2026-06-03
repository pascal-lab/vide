#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindTargetList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BindTargetList<'a> {
    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn targets(&self) -> SeparatedList<'a, Name<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for BindTargetList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BIND_TARGET_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaggedUnionExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TaggedUnionExpression<'a> {
    #[inline]
    pub fn tagged(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn member(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(2usize).and_then(Expression::cast)
    }
}
impl<'a> AstNode<'a> for TaggedUnionExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TAGGED_UNION_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExternInterfaceMethod<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExternInterfaceMethod<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn extern_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn fork_join(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn prototype(&self) -> FunctionPrototype<'a> {
        self.syntax().child_node(3usize).and_then(FunctionPrototype::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for ExternInterfaceMethod<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXTERN_INTERFACE_METHOD
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenthesizedSequenceExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenthesizedSequenceExpr<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(1usize).and_then(SequenceExpr::cast).unwrap()
    }

    #[inline]
    pub fn match_list(&self) -> Option<SequenceMatchList<'a>> {
        self.syntax().child_node(2usize).and_then(SequenceMatchList::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn repetition(&self) -> Option<SequenceRepetition<'a>> {
        self.syntax().child_node(4usize).and_then(SequenceRepetition::cast)
    }
}
impl<'a> AstNode<'a> for ParenthesizedSequenceExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_SEQUENCE_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventExpression<'a> {
    SignalEventExpression(SignalEventExpression<'a>),
    ParenthesizedEventExpression(ParenthesizedEventExpression<'a>),
    BinaryEventExpression(BinaryEventExpression<'a>),
}
impl<'a> EventExpression<'a> {
    #[inline]
    pub fn as_signal_event_expression(self) -> Option<SignalEventExpression<'a>> {
        match self {
            Self::SignalEventExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_parenthesized_event_expression(self) -> Option<ParenthesizedEventExpression<'a>> {
        match self {
            Self::ParenthesizedEventExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_event_expression(self) -> Option<BinaryEventExpression<'a>> {
        match self {
            Self::BinaryEventExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for EventExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::SignalEventExpression(node) => node.syntax(),
            Self::ParenthesizedEventExpression(node) => node.syntax(),
            Self::BinaryEventExpression(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIGNAL_EVENT_EXPRESSION
            || kind == SyntaxKind::PARENTHESIZED_EVENT_EXPRESSION
            || kind == SyntaxKind::BINARY_EVENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::SIGNAL_EVENT_EXPRESSION => {
                Some(Self::SignalEventExpression(SignalEventExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::PARENTHESIZED_EVENT_EXPRESSION => Some(Self::ParenthesizedEventExpression(
                ParenthesizedEventExpression::cast(syntax).unwrap(),
            )),
            SyntaxKind::BINARY_EVENT_EXPRESSION => {
                Some(Self::BinaryEventExpression(BinaryEventExpression::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DriveStrength<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DriveStrength<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn strength_0(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn comma(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn strength_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for DriveStrength<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DRIVE_STRENGTH
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ForeachLoopList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ForeachLoopList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn array_name(&self) -> Name<'a> {
        self.syntax().child_node(1usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn loop_variables(&self) -> SeparatedList<'a, Name<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for ForeachLoopList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FOREACH_LOOP_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Delay3<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> Delay3<'a> {
    #[inline]
    pub fn hash(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn delay_1(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn comma_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn delay_2(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(4usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn comma_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn delay_3(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(6usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }
}
impl<'a> AstNode<'a> for Delay3<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DELAY_3
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaggedPattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TaggedPattern<'a> {
    #[inline]
    pub fn tagged(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn member_name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn pattern(&self) -> Option<Pattern<'a>> {
        self.syntax().child_node(2usize).and_then(Pattern::cast)
    }
}
impl<'a> AstNode<'a> for TaggedPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TAGGED_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedLabel<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NamedLabel<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for NamedLabel<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_LABEL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StructurePatternMember<'a> {
    OrderedStructurePatternMember(OrderedStructurePatternMember<'a>),
    NamedStructurePatternMember(NamedStructurePatternMember<'a>),
}
impl<'a> StructurePatternMember<'a> {
    #[inline]
    pub fn as_ordered_structure_pattern_member(self) -> Option<OrderedStructurePatternMember<'a>> {
        match self {
            Self::OrderedStructurePatternMember(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_named_structure_pattern_member(self) -> Option<NamedStructurePatternMember<'a>> {
        match self {
            Self::NamedStructurePatternMember(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for StructurePatternMember<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::OrderedStructurePatternMember(node) => node.syntax(),
            Self::NamedStructurePatternMember(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ORDERED_STRUCTURE_PATTERN_MEMBER
            || kind == SyntaxKind::NAMED_STRUCTURE_PATTERN_MEMBER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ORDERED_STRUCTURE_PATTERN_MEMBER => {
                Some(Self::OrderedStructurePatternMember(
                    OrderedStructurePatternMember::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::NAMED_STRUCTURE_PATTERN_MEMBER => Some(Self::NamedStructurePatternMember(
                NamedStructurePatternMember::cast(syntax).unwrap(),
            )),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LetDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LetDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn let_(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn port_list(&self) -> Option<AssertionItemPortList<'a>> {
        self.syntax().child_node(3usize).and_then(AssertionItemPortList::cast)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(5usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }
}
impl<'a> AstNode<'a> for LetDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LET_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EdgeControlSpecifier<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EdgeControlSpecifier<'a> {
    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn descriptors(&self) -> SeparatedList<'a, EdgeDescriptor<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for EdgeControlSpecifier<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EDGE_CONTROL_SPECIFIER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpecifyBlock<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SpecifyBlock<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn specify(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endspecify(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for SpecifyBlock<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SPECIFY_BLOCK
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NetAlias<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NetAlias<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn nets(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for NetAlias<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NET_ALIAS
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeywordName<'a> {
    ConstructorName(SyntaxNode<'a>),
    RootScope(SyntaxNode<'a>),
    UnitScope(SyntaxNode<'a>),
    ThisHandle(SyntaxNode<'a>),
    ArrayUniqueMethod(SyntaxNode<'a>),
    ArrayOrMethod(SyntaxNode<'a>),
    ArrayAndMethod(SyntaxNode<'a>),
    LocalScope(SyntaxNode<'a>),
    SuperHandle(SyntaxNode<'a>),
    ArrayXorMethod(SyntaxNode<'a>),
}
impl<'a> KeywordName<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn as_constructor_name(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ConstructorName(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_root_scope(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::RootScope(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unit_scope(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnitScope(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_this_handle(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ThisHandle(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_unique_method(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ArrayUniqueMethod(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_or_method(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ArrayOrMethod(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_and_method(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ArrayAndMethod(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_local_scope(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LocalScope(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_super_handle(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::SuperHandle(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_xor_method(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ArrayXorMethod(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for KeywordName<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ConstructorName(node) => *node,
            Self::RootScope(node) => *node,
            Self::UnitScope(node) => *node,
            Self::ThisHandle(node) => *node,
            Self::ArrayUniqueMethod(node) => *node,
            Self::ArrayOrMethod(node) => *node,
            Self::ArrayAndMethod(node) => *node,
            Self::LocalScope(node) => *node,
            Self::SuperHandle(node) => *node,
            Self::ArrayXorMethod(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONSTRUCTOR_NAME
            || kind == SyntaxKind::ROOT_SCOPE
            || kind == SyntaxKind::UNIT_SCOPE
            || kind == SyntaxKind::THIS_HANDLE
            || kind == SyntaxKind::ARRAY_UNIQUE_METHOD
            || kind == SyntaxKind::ARRAY_OR_METHOD
            || kind == SyntaxKind::ARRAY_AND_METHOD
            || kind == SyntaxKind::LOCAL_SCOPE
            || kind == SyntaxKind::SUPER_HANDLE
            || kind == SyntaxKind::ARRAY_XOR_METHOD
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::CONSTRUCTOR_NAME => Some(Self::ConstructorName(syntax)),
            SyntaxKind::ROOT_SCOPE => Some(Self::RootScope(syntax)),
            SyntaxKind::UNIT_SCOPE => Some(Self::UnitScope(syntax)),
            SyntaxKind::THIS_HANDLE => Some(Self::ThisHandle(syntax)),
            SyntaxKind::ARRAY_UNIQUE_METHOD => Some(Self::ArrayUniqueMethod(syntax)),
            SyntaxKind::ARRAY_OR_METHOD => Some(Self::ArrayOrMethod(syntax)),
            SyntaxKind::ARRAY_AND_METHOD => Some(Self::ArrayAndMethod(syntax)),
            SyntaxKind::LOCAL_SCOPE => Some(Self::LocalScope(syntax)),
            SyntaxKind::SUPER_HANDLE => Some(Self::SuperHandle(syntax)),
            SyntaxKind::ARRAY_XOR_METHOD => Some(Self::ArrayXorMethod(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnumType<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EnumType<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn base_type(&self) -> Option<DataType<'a>> {
        self.syntax().child_node(1usize).and_then(DataType::cast)
    }

    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn members(&self) -> SeparatedList<'a, Declarator<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn dimensions(&self) -> SyntaxList<'a, VariableDimension<'a>> {
        self.syntax().child_node(5usize).and_then(SyntaxList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for EnumType<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ENUM_TYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BadExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BadExpression<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for BadExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BAD_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImplicitAnsiPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ImplicitAnsiPort<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn header(&self) -> PortHeader<'a> {
        self.syntax().child_node(1usize).and_then(PortHeader::cast).unwrap()
    }

    #[inline]
    pub fn declarator(&self) -> Declarator<'a> {
        self.syntax().child_node(2usize).and_then(Declarator::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ImplicitAnsiPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMPLICIT_ANSI_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockingDirection<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClockingDirection<'a> {
    #[inline]
    pub fn input(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn input_skew(&self) -> Option<ClockingSkew<'a>> {
        self.syntax().child_node(1usize).and_then(ClockingSkew::cast)
    }

    #[inline]
    pub fn output(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn output_skew(&self) -> Option<ClockingSkew<'a>> {
        self.syntax().child_node(3usize).and_then(ClockingSkew::cast)
    }
}
impl<'a> AstNode<'a> for ClockingDirection<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLOCKING_DIRECTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpSimpleField<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UdpSimpleField<'a> {
    #[inline]
    pub fn field(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for UdpSimpleField<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_SIMPLE_FIELD
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArrayOrRandomizeMethodExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ArrayOrRandomizeMethodExpression<'a> {
    #[inline]
    pub fn method(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn with(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn args(&self) -> Option<ParenExpressionList<'a>> {
        self.syntax().child_node(2usize).and_then(ParenExpressionList::cast)
    }

    #[inline]
    pub fn constraints(&self) -> Option<ConstraintBlock<'a>> {
        self.syntax().child_node(3usize).and_then(ConstraintBlock::cast)
    }
}
impl<'a> AstNode<'a> for ArrayOrRandomizeMethodExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ARRAY_OR_RANDOMIZE_METHOD_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionConstraint<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExpressionConstraint<'a> {
    #[inline]
    pub fn soft(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ExpressionConstraint<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPRESSION_CONSTRAINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntersectClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IntersectClause<'a> {
    #[inline]
    pub fn intersect(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ranges(&self) -> RangeList<'a> {
        self.syntax().child_node(1usize).and_then(RangeList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for IntersectClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INTERSECT_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropertyExpr<'a> {
    BinaryPropertyExpr(BinaryPropertyExpr<'a>),
    UnaryPropertyExpr(UnaryPropertyExpr<'a>),
    CasePropertyExpr(CasePropertyExpr<'a>),
    SimplePropertyExpr(SimplePropertyExpr<'a>),
    ClockingPropertyExpr(ClockingPropertyExpr<'a>),
    UnarySelectPropertyExpr(UnarySelectPropertyExpr<'a>),
    ParenthesizedPropertyExpr(ParenthesizedPropertyExpr<'a>),
    AcceptOnPropertyExpr(AcceptOnPropertyExpr<'a>),
    ConditionalPropertyExpr(ConditionalPropertyExpr<'a>),
    StrongWeakPropertyExpr(StrongWeakPropertyExpr<'a>),
}
impl<'a> PropertyExpr<'a> {
    #[inline]
    pub fn as_binary_property_expr(self) -> Option<BinaryPropertyExpr<'a>> {
        match self {
            Self::BinaryPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_property_expr(self) -> Option<UnaryPropertyExpr<'a>> {
        match self {
            Self::UnaryPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_case_property_expr(self) -> Option<CasePropertyExpr<'a>> {
        match self {
            Self::CasePropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_simple_property_expr(self) -> Option<SimplePropertyExpr<'a>> {
        match self {
            Self::SimplePropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_clocking_property_expr(self) -> Option<ClockingPropertyExpr<'a>> {
        match self {
            Self::ClockingPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_select_property_expr(self) -> Option<UnarySelectPropertyExpr<'a>> {
        match self {
            Self::UnarySelectPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_parenthesized_property_expr(self) -> Option<ParenthesizedPropertyExpr<'a>> {
        match self {
            Self::ParenthesizedPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_accept_on_property_expr(self) -> Option<AcceptOnPropertyExpr<'a>> {
        match self {
            Self::AcceptOnPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_conditional_property_expr(self) -> Option<ConditionalPropertyExpr<'a>> {
        match self {
            Self::ConditionalPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_strong_weak_property_expr(self) -> Option<StrongWeakPropertyExpr<'a>> {
        match self {
            Self::StrongWeakPropertyExpr(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::BinaryPropertyExpr(node) => node.syntax(),
            Self::UnaryPropertyExpr(node) => node.syntax(),
            Self::CasePropertyExpr(node) => node.syntax(),
            Self::SimplePropertyExpr(node) => node.syntax(),
            Self::ClockingPropertyExpr(node) => node.syntax(),
            Self::UnarySelectPropertyExpr(node) => node.syntax(),
            Self::ParenthesizedPropertyExpr(node) => node.syntax(),
            Self::AcceptOnPropertyExpr(node) => node.syntax(),
            Self::ConditionalPropertyExpr(node) => node.syntax(),
            Self::StrongWeakPropertyExpr(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMPLIES_PROPERTY_EXPR
            || kind == SyntaxKind::UNARY_PROPERTY_EXPR
            || kind == SyntaxKind::CASE_PROPERTY_EXPR
            || kind == SyntaxKind::SIMPLE_PROPERTY_EXPR
            || kind == SyntaxKind::CLOCKING_PROPERTY_EXPR
            || kind == SyntaxKind::UNARY_SELECT_PROPERTY_EXPR
            || kind == SyntaxKind::UNTIL_WITH_PROPERTY_EXPR
            || kind == SyntaxKind::S_UNTIL_PROPERTY_EXPR
            || kind == SyntaxKind::UNTIL_PROPERTY_EXPR
            || kind == SyntaxKind::PARENTHESIZED_PROPERTY_EXPR
            || kind == SyntaxKind::ACCEPT_ON_PROPERTY_EXPR
            || kind == SyntaxKind::OR_PROPERTY_EXPR
            || kind == SyntaxKind::AND_PROPERTY_EXPR
            || kind == SyntaxKind::S_UNTIL_WITH_PROPERTY_EXPR
            || kind == SyntaxKind::FOLLOWED_BY_PROPERTY_EXPR
            || kind == SyntaxKind::IFF_PROPERTY_EXPR
            || kind == SyntaxKind::IMPLICATION_PROPERTY_EXPR
            || kind == SyntaxKind::CONDITIONAL_PROPERTY_EXPR
            || kind == SyntaxKind::STRONG_WEAK_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::IMPLIES_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_PROPERTY_EXPR => {
                Some(Self::UnaryPropertyExpr(UnaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::CASE_PROPERTY_EXPR => {
                Some(Self::CasePropertyExpr(CasePropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::SIMPLE_PROPERTY_EXPR => {
                Some(Self::SimplePropertyExpr(SimplePropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::CLOCKING_PROPERTY_EXPR => {
                Some(Self::ClockingPropertyExpr(ClockingPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_SELECT_PROPERTY_EXPR => {
                Some(Self::UnarySelectPropertyExpr(UnarySelectPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::UNTIL_WITH_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::S_UNTIL_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::UNTIL_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::PARENTHESIZED_PROPERTY_EXPR => Some(Self::ParenthesizedPropertyExpr(
                ParenthesizedPropertyExpr::cast(syntax).unwrap(),
            )),
            SyntaxKind::ACCEPT_ON_PROPERTY_EXPR => {
                Some(Self::AcceptOnPropertyExpr(AcceptOnPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::OR_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::AND_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::S_UNTIL_WITH_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::FOLLOWED_BY_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::IFF_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::IMPLICATION_PROPERTY_EXPR => {
                Some(Self::BinaryPropertyExpr(BinaryPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::CONDITIONAL_PROPERTY_EXPR => {
                Some(Self::ConditionalPropertyExpr(ConditionalPropertyExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::STRONG_WEAK_PROPERTY_EXPR => {
                Some(Self::StrongWeakPropertyExpr(StrongWeakPropertyExpr::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimplePropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SimplePropertyExpr<'a> {
    #[inline]
    pub fn expr(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(0usize).and_then(SequenceExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for SimplePropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIMPLE_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParameterPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParameterPortList<'a> {
    #[inline]
    pub fn hash(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn declarations(&self) -> SeparatedList<'a, ParameterDeclarationBase<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ParameterPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARAMETER_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProceduralDeassignStatement<'a> {
    ProceduralReleaseStatement(SyntaxNode<'a>),
    ProceduralDeassignStatement(SyntaxNode<'a>),
}
impl<'a> ProceduralDeassignStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn variable(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn as_procedural_release_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ProceduralReleaseStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_procedural_deassign_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ProceduralDeassignStatement(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ProceduralDeassignStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ProceduralReleaseStatement(node) => *node,
            Self::ProceduralDeassignStatement(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PROCEDURAL_RELEASE_STATEMENT
            || kind == SyntaxKind::PROCEDURAL_DEASSIGN_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PROCEDURAL_RELEASE_STATEMENT => {
                Some(Self::ProceduralReleaseStatement(syntax))
            }
            SyntaxKind::PROCEDURAL_DEASSIGN_STATEMENT => {
                Some(Self::ProceduralDeassignStatement(syntax))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GenerateRegion<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> GenerateRegion<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endgenerate(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for GenerateRegion<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::GENERATE_REGION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StreamExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StreamExpression<'a> {
    #[inline]
    pub fn expression(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn with_range(&self) -> Option<StreamExpressionWithRange<'a>> {
        self.syntax().child_node(1usize).and_then(StreamExpressionWithRange::cast)
    }
}
impl<'a> AstNode<'a> for StreamExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STREAM_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyMember<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EmptyMember<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn qualifiers(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for EmptyMember<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_MEMBER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageIffClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CoverageIffClause<'a> {
    #[inline]
    pub fn iff(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for CoverageIffClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVERAGE_IFF_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PortList<'a> {
    WildcardPortList(WildcardPortList<'a>),
    AnsiPortList(AnsiPortList<'a>),
    NonAnsiPortList(NonAnsiPortList<'a>),
}
impl<'a> PortList<'a> {
    #[inline]
    pub fn as_wildcard_port_list(self) -> Option<WildcardPortList<'a>> {
        match self {
            Self::WildcardPortList(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ansi_port_list(self) -> Option<AnsiPortList<'a>> {
        match self {
            Self::AnsiPortList(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_non_ansi_port_list(self) -> Option<NonAnsiPortList<'a>> {
        match self {
            Self::NonAnsiPortList(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::WildcardPortList(node) => node.syntax(),
            Self::AnsiPortList(node) => node.syntax(),
            Self::NonAnsiPortList(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WILDCARD_PORT_LIST
            || kind == SyntaxKind::ANSI_PORT_LIST
            || kind == SyntaxKind::NON_ANSI_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::WILDCARD_PORT_LIST => {
                Some(Self::WildcardPortList(WildcardPortList::cast(syntax).unwrap()))
            }
            SyntaxKind::ANSI_PORT_LIST => {
                Some(Self::AnsiPortList(AnsiPortList::cast(syntax).unwrap()))
            }
            SyntaxKind::NON_ANSI_PORT_LIST => {
                Some(Self::NonAnsiPortList(NonAnsiPortList::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConcurrentAssertionStatement<'a> {
    CoverPropertyStatement(SyntaxNode<'a>),
    AssertPropertyStatement(SyntaxNode<'a>),
    RestrictPropertyStatement(SyntaxNode<'a>),
    ExpectPropertyStatement(SyntaxNode<'a>),
    AssumePropertyStatement(SyntaxNode<'a>),
    CoverSequenceStatement(SyntaxNode<'a>),
}
impl<'a> ConcurrentAssertionStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn property_or_sequence(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn property_spec(&self) -> PropertySpec<'a> {
        self.syntax().child_node(5usize).and_then(PropertySpec::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn action(&self) -> ActionBlock<'a> {
        self.syntax().child_node(7usize).and_then(ActionBlock::cast).unwrap()
    }

    #[inline]
    pub fn as_cover_property_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::CoverPropertyStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_assert_property_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AssertPropertyStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_restrict_property_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::RestrictPropertyStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_expect_property_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ExpectPropertyStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_assume_property_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AssumePropertyStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_cover_sequence_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::CoverSequenceStatement(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ConcurrentAssertionStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::CoverPropertyStatement(node) => *node,
            Self::AssertPropertyStatement(node) => *node,
            Self::RestrictPropertyStatement(node) => *node,
            Self::ExpectPropertyStatement(node) => *node,
            Self::AssumePropertyStatement(node) => *node,
            Self::CoverSequenceStatement(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVER_PROPERTY_STATEMENT
            || kind == SyntaxKind::ASSERT_PROPERTY_STATEMENT
            || kind == SyntaxKind::RESTRICT_PROPERTY_STATEMENT
            || kind == SyntaxKind::EXPECT_PROPERTY_STATEMENT
            || kind == SyntaxKind::ASSUME_PROPERTY_STATEMENT
            || kind == SyntaxKind::COVER_SEQUENCE_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::COVER_PROPERTY_STATEMENT => Some(Self::CoverPropertyStatement(syntax)),
            SyntaxKind::ASSERT_PROPERTY_STATEMENT => Some(Self::AssertPropertyStatement(syntax)),
            SyntaxKind::RESTRICT_PROPERTY_STATEMENT => {
                Some(Self::RestrictPropertyStatement(syntax))
            }
            SyntaxKind::EXPECT_PROPERTY_STATEMENT => Some(Self::ExpectPropertyStatement(syntax)),
            SyntaxKind::ASSUME_PROPERTY_STATEMENT => Some(Self::AssumePropertyStatement(syntax)),
            SyntaxKind::COVER_SEQUENCE_STATEMENT => Some(Self::CoverSequenceStatement(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StrongWeakPropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StrongWeakPropertyExpr<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(2usize).and_then(SequenceExpr::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for StrongWeakPropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STRONG_WEAK_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExternUdpDecl<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExternUdpDecl<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn extern_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn actual_attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn primitive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn port_list(&self) -> UdpPortList<'a> {
        self.syntax().child_node(5usize).and_then(UdpPortList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ExternUdpDecl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXTERN_UDP_DECL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockingPropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClockingPropertyExpr<'a> {
    #[inline]
    pub fn event(&self) -> TimingControl<'a> {
        self.syntax().child_node(0usize).and_then(TimingControl::cast).unwrap()
    }

    #[inline]
    pub fn expr(&self) -> Option<PropertyExpr<'a>> {
        self.syntax().child_node(1usize).and_then(PropertyExpr::cast)
    }
}
impl<'a> AstNode<'a> for ClockingPropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLOCKING_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PrimitiveInstantiation<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PrimitiveInstantiation<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn type_(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn strength(&self) -> Option<NetStrength<'a>> {
        self.syntax().child_node(2usize).and_then(NetStrength::cast)
    }

    #[inline]
    pub fn delay(&self) -> Option<TimingControl<'a>> {
        self.syntax().child_node(3usize).and_then(TimingControl::cast)
    }

    #[inline]
    pub fn instances(&self) -> SeparatedList<'a, HierarchicalInstance<'a>> {
        self.syntax().child_node(4usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for PrimitiveInstantiation<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PRIMITIVE_INSTANTIATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalVariableDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LocalVariableDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn var(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(2usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, Declarator<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for LocalVariableDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LOCAL_VARIABLE_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LibraryMap<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LibraryMap<'a> {
    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end_of_file(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for LibraryMap<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LIBRARY_MAP
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacroFormalArgument<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MacroFormalArgument<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn default_value(&self) -> Option<MacroArgumentDefault<'a>> {
        self.syntax().child_node(1usize).and_then(MacroArgumentDefault::cast)
    }
}
impl<'a> AstNode<'a> for MacroFormalArgument<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MACRO_FORMAL_ARGUMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LineDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn line_number(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn file_name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn level(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for LineDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LINE_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyTimingCheckArg<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EmptyTimingCheckArg<'a> {
    #[inline]
    pub fn placeholder(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for EmptyTimingCheckArg<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_TIMING_CHECK_ARG
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructuredAssignmentPattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StructuredAssignmentPattern<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, AssignmentPatternItem<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for StructuredAssignmentPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STRUCTURED_ASSIGNMENT_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ForwardTypedefDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ForwardTypedefDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn typedef_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn type_restriction(&self) -> Option<ForwardTypeRestriction<'a>> {
        self.syntax().child_node(2usize).and_then(ForwardTypeRestriction::cast)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for ForwardTypedefDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FORWARD_TYPEDEF_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeAssignment<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TypeAssignment<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn assignment(&self) -> Option<EqualsTypeClause<'a>> {
        self.syntax().child_node(1usize).and_then(EqualsTypeClause::cast)
    }
}
impl<'a> AstNode<'a> for TypeAssignment<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TYPE_ASSIGNMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RandCaseStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RandCaseStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn rand_case(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, RandCaseItem<'a>> {
        self.syntax().child_node(3usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end_case(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for RandCaseStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RAND_CASE_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemberAccessExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MemberAccessExpression<'a> {
    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for MemberAccessExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MEMBER_ACCESS_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PortExpression<'a> {
    PortConcatenation(PortConcatenation<'a>),
    PortReference(PortReference<'a>),
}
impl<'a> PortExpression<'a> {
    #[inline]
    pub fn as_port_concatenation(self) -> Option<PortConcatenation<'a>> {
        match self {
            Self::PortConcatenation(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_port_reference(self) -> Option<PortReference<'a>> {
        match self {
            Self::PortReference(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PortExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::PortConcatenation(node) => node.syntax(),
            Self::PortReference(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PORT_CONCATENATION || kind == SyntaxKind::PORT_REFERENCE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PORT_CONCATENATION => {
                Some(Self::PortConcatenation(PortConcatenation::cast(syntax).unwrap()))
            }
            SyntaxKind::PORT_REFERENCE => {
                Some(Self::PortReference(PortReference::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DPIImport<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DPIImport<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn spec_string(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn property(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn c_identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn method(&self) -> FunctionPrototype<'a> {
        self.syntax().child_node(6usize).and_then(FunctionPrototype::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }
}
impl<'a> AstNode<'a> for DPIImport<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DPI_IMPORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimpleAssignmentPattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SimpleAssignmentPattern<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for SimpleAssignmentPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIMPLE_ASSIGNMENT_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsCase<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RsCase<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, RsCaseItem<'a>> {
        self.syntax().child_node(4usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endcase(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for RsCase<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_CASE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WaitStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WaitStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn wait(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(4usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(6usize).and_then(Statement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for WaitStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WAIT_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JumpStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> JumpStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn break_or_continue(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for JumpStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::JUMP_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PulseStyleDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PulseStyleDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn inputs(&self) -> SeparatedList<'a, Name<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for PulseStyleDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PULSE_STYLE_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CovergroupDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CovergroupDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn covergroup(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn extends(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn port_list(&self) -> Option<FunctionPortList<'a>> {
        self.syntax().child_node(4usize).and_then(FunctionPortList::cast)
    }

    #[inline]
    pub fn event(&self) -> Option<HybridNode<'a>> {
        self.syntax().child_node(5usize).and_then(HybridNode::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(7usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endgroup(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(9usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for CovergroupDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVERGROUP_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinarySequenceExpr<'a> {
    IntersectSequenceExpr(SyntaxNode<'a>),
    OrSequenceExpr(SyntaxNode<'a>),
    AndSequenceExpr(SyntaxNode<'a>),
    ThroughoutSequenceExpr(SyntaxNode<'a>),
    WithinSequenceExpr(SyntaxNode<'a>),
}
impl<'a> BinarySequenceExpr<'a> {
    #[inline]
    pub fn left(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(0usize).and_then(SequenceExpr::cast).unwrap()
    }

    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(2usize).and_then(SequenceExpr::cast).unwrap()
    }

    #[inline]
    pub fn as_intersect_sequence_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::IntersectSequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_or_sequence_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::OrSequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_and_sequence_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AndSequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_throughout_sequence_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ThroughoutSequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_within_sequence_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::WithinSequenceExpr(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for BinarySequenceExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::IntersectSequenceExpr(node) => *node,
            Self::OrSequenceExpr(node) => *node,
            Self::AndSequenceExpr(node) => *node,
            Self::ThroughoutSequenceExpr(node) => *node,
            Self::WithinSequenceExpr(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INTERSECT_SEQUENCE_EXPR
            || kind == SyntaxKind::OR_SEQUENCE_EXPR
            || kind == SyntaxKind::AND_SEQUENCE_EXPR
            || kind == SyntaxKind::THROUGHOUT_SEQUENCE_EXPR
            || kind == SyntaxKind::WITHIN_SEQUENCE_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::INTERSECT_SEQUENCE_EXPR => Some(Self::IntersectSequenceExpr(syntax)),
            SyntaxKind::OR_SEQUENCE_EXPR => Some(Self::OrSequenceExpr(syntax)),
            SyntaxKind::AND_SEQUENCE_EXPR => Some(Self::AndSequenceExpr(syntax)),
            SyntaxKind::THROUGHOUT_SEQUENCE_EXPR => Some(Self::ThroughoutSequenceExpr(syntax)),
            SyntaxKind::WITHIN_SEQUENCE_EXPR => Some(Self::WithinSequenceExpr(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssignmentPattern<'a> {
    ReplicatedAssignmentPattern(ReplicatedAssignmentPattern<'a>),
    StructuredAssignmentPattern(StructuredAssignmentPattern<'a>),
    SimpleAssignmentPattern(SimpleAssignmentPattern<'a>),
}
impl<'a> AssignmentPattern<'a> {
    #[inline]
    pub fn as_replicated_assignment_pattern(self) -> Option<ReplicatedAssignmentPattern<'a>> {
        match self {
            Self::ReplicatedAssignmentPattern(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_structured_assignment_pattern(self) -> Option<StructuredAssignmentPattern<'a>> {
        match self {
            Self::StructuredAssignmentPattern(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_simple_assignment_pattern(self) -> Option<SimpleAssignmentPattern<'a>> {
        match self {
            Self::SimpleAssignmentPattern(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for AssignmentPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ReplicatedAssignmentPattern(node) => node.syntax(),
            Self::StructuredAssignmentPattern(node) => node.syntax(),
            Self::SimpleAssignmentPattern(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::REPLICATED_ASSIGNMENT_PATTERN
            || kind == SyntaxKind::STRUCTURED_ASSIGNMENT_PATTERN
            || kind == SyntaxKind::SIMPLE_ASSIGNMENT_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::REPLICATED_ASSIGNMENT_PATTERN => Some(Self::ReplicatedAssignmentPattern(
                ReplicatedAssignmentPattern::cast(syntax).unwrap(),
            )),
            SyntaxKind::STRUCTURED_ASSIGNMENT_PATTERN => Some(Self::StructuredAssignmentPattern(
                StructuredAssignmentPattern::cast(syntax).unwrap(),
            )),
            SyntaxKind::SIMPLE_ASSIGNMENT_PATTERN => {
                Some(Self::SimpleAssignmentPattern(SimpleAssignmentPattern::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceConfigRule<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> InstanceConfigRule<'a> {
    #[inline]
    pub fn instance(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn top_module(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn instance_names(&self) -> SyntaxList<'a, ConfigInstanceIdentifier<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn rule_clause(&self) -> ConfigRuleClause<'a> {
        self.syntax().child_node(3usize).and_then(ConfigRuleClause::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for InstanceConfigRule<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INSTANCE_CONFIG_RULE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExpressionStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ExpressionStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPRESSION_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WildcardPattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WildcardPattern<'a> {
    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn star(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for WildcardPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WILDCARD_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MultipleConcatenationExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MultipleConcatenationExpression<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expression(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn concatenation(&self) -> ConcatenationExpression<'a> {
        self.syntax().child_node(2usize).and_then(ConcatenationExpression::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for MultipleConcatenationExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MULTIPLE_CONCATENATION_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PortConnection<'a> {
    NamedPortConnection(NamedPortConnection<'a>),
    WildcardPortConnection(WildcardPortConnection<'a>),
    OrderedPortConnection(OrderedPortConnection<'a>),
    EmptyPortConnection(EmptyPortConnection<'a>),
}
impl<'a> PortConnection<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn as_named_port_connection(self) -> Option<NamedPortConnection<'a>> {
        match self {
            Self::NamedPortConnection(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_wildcard_port_connection(self) -> Option<WildcardPortConnection<'a>> {
        match self {
            Self::WildcardPortConnection(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ordered_port_connection(self) -> Option<OrderedPortConnection<'a>> {
        match self {
            Self::OrderedPortConnection(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_empty_port_connection(self) -> Option<EmptyPortConnection<'a>> {
        match self {
            Self::EmptyPortConnection(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PortConnection<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::NamedPortConnection(node) => node.syntax(),
            Self::WildcardPortConnection(node) => node.syntax(),
            Self::OrderedPortConnection(node) => node.syntax(),
            Self::EmptyPortConnection(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_PORT_CONNECTION
            || kind == SyntaxKind::WILDCARD_PORT_CONNECTION
            || kind == SyntaxKind::ORDERED_PORT_CONNECTION
            || kind == SyntaxKind::EMPTY_PORT_CONNECTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::NAMED_PORT_CONNECTION => {
                Some(Self::NamedPortConnection(NamedPortConnection::cast(syntax).unwrap()))
            }
            SyntaxKind::WILDCARD_PORT_CONNECTION => {
                Some(Self::WildcardPortConnection(WildcardPortConnection::cast(syntax).unwrap()))
            }
            SyntaxKind::ORDERED_PORT_CONNECTION => {
                Some(Self::OrderedPortConnection(OrderedPortConnection::cast(syntax).unwrap()))
            }
            SyntaxKind::EMPTY_PORT_CONNECTION => {
                Some(Self::EmptyPortConnection(EmptyPortConnection::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CheckerInstantiation<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CheckerInstantiation<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn type_(&self) -> Name<'a> {
        self.syntax().child_node(1usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn parameters(&self) -> Option<ParameterValueAssignment<'a>> {
        self.syntax().child_node(2usize).and_then(ParameterValueAssignment::cast)
    }

    #[inline]
    pub fn instances(&self) -> SeparatedList<'a, HierarchicalInstance<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for CheckerInstantiation<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CHECKER_INSTANTIATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MinTypMaxExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MinTypMaxExpression<'a> {
    #[inline]
    pub fn min(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn colon_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn typ(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn colon_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn max(&self) -> Expression<'a> {
        self.syntax().child_node(4usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for MinTypMaxExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MIN_TYP_MAX_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionTimingCheckArg<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExpressionTimingCheckArg<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ExpressionTimingCheckArg<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPRESSION_TIMING_CHECK_ARG
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RangeCoverageBinInitializer<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RangeCoverageBinInitializer<'a> {
    #[inline]
    pub fn ranges(&self) -> RangeList<'a> {
        self.syntax().child_node(0usize).and_then(RangeList::cast).unwrap()
    }

    #[inline]
    pub fn with_clause(&self) -> Option<WithClause<'a>> {
        self.syntax().child_node(1usize).and_then(WithClause::cast)
    }
}
impl<'a> AstNode<'a> for RangeCoverageBinInitializer<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RANGE_COVERAGE_BIN_INITIALIZER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RangeDimensionSpecifier<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RangeDimensionSpecifier<'a> {
    #[inline]
    pub fn selector(&self) -> Selector<'a> {
        self.syntax().child_node(0usize).and_then(Selector::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for RangeDimensionSpecifier<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RANGE_DIMENSION_SPECIFIER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RandSequenceStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RandSequenceStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn randsequence(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn first_production(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn productions(&self) -> SyntaxList<'a, Production<'a>> {
        self.syntax().child_node(6usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endsequence(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }
}
impl<'a> AstNode<'a> for RandSequenceStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RAND_SEQUENCE_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionalExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConditionalExpression<'a> {
    #[inline]
    pub fn predicate(&self) -> ConditionalPredicate<'a> {
        self.syntax().child_node(0usize).and_then(ConditionalPredicate::cast).unwrap()
    }

    #[inline]
    pub fn question(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn right(&self) -> Expression<'a> {
        self.syntax().child_node(5usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ConditionalExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONDITIONAL_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModuleHeader<'a> {
    ModuleHeader(SyntaxNode<'a>),
    ProgramHeader(SyntaxNode<'a>),
    InterfaceHeader(SyntaxNode<'a>),
    PackageHeader(SyntaxNode<'a>),
}
impl<'a> ModuleHeader<'a> {
    #[inline]
    pub fn module_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn lifetime(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn imports(&self) -> SyntaxList<'a, PackageImportDeclaration<'a>> {
        self.syntax().child_node(3usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn parameters(&self) -> Option<ParameterPortList<'a>> {
        self.syntax().child_node(4usize).and_then(ParameterPortList::cast)
    }

    #[inline]
    pub fn ports(&self) -> Option<PortList<'a>> {
        self.syntax().child_node(5usize).and_then(PortList::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn as_module_header(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ModuleHeader(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_program_header(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ProgramHeader(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_interface_header(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::InterfaceHeader(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_package_header(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::PackageHeader(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ModuleHeader<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ModuleHeader(node) => *node,
            Self::ProgramHeader(node) => *node,
            Self::InterfaceHeader(node) => *node,
            Self::PackageHeader(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODULE_HEADER
            || kind == SyntaxKind::PROGRAM_HEADER
            || kind == SyntaxKind::INTERFACE_HEADER
            || kind == SyntaxKind::PACKAGE_HEADER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::MODULE_HEADER => Some(Self::ModuleHeader(syntax)),
            SyntaxKind::PROGRAM_HEADER => Some(Self::ProgramHeader(syntax)),
            SyntaxKind::INTERFACE_HEADER => Some(Self::InterfaceHeader(syntax)),
            SyntaxKind::PACKAGE_HEADER => Some(Self::PackageHeader(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventTriggerStatement<'a> {
    BlockingEventTriggerStatement(SyntaxNode<'a>),
    NonblockingEventTriggerStatement(SyntaxNode<'a>),
}
impl<'a> EventTriggerStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn trigger(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn timing(&self) -> Option<TimingControl<'a>> {
        self.syntax().child_node(3usize).and_then(TimingControl::cast)
    }

    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(4usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn as_blocking_event_trigger_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::BlockingEventTriggerStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_nonblocking_event_trigger_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::NonblockingEventTriggerStatement(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for EventTriggerStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::BlockingEventTriggerStatement(node) => *node,
            Self::NonblockingEventTriggerStatement(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BLOCKING_EVENT_TRIGGER_STATEMENT
            || kind == SyntaxKind::NONBLOCKING_EVENT_TRIGGER_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::BLOCKING_EVENT_TRIGGER_STATEMENT => {
                Some(Self::BlockingEventTriggerStatement(syntax))
            }
            SyntaxKind::NONBLOCKING_EVENT_TRIGGER_STATEMENT => {
                Some(Self::NonblockingEventTriggerStatement(syntax))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CastExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CastExpression<'a> {
    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn apostrophe(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> ParenthesizedExpression<'a> {
        self.syntax().child_node(2usize).and_then(ParenthesizedExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for CastExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CAST_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModportSubroutinePort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ModportSubroutinePort<'a> {
    #[inline]
    pub fn prototype(&self) -> FunctionPrototype<'a> {
        self.syntax().child_node(0usize).and_then(FunctionPrototype::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ModportSubroutinePort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_SUBROUTINE_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntegerType<'a> {
    LongIntType(SyntaxNode<'a>),
    IntType(SyntaxNode<'a>),
    LogicType(SyntaxNode<'a>),
    BitType(SyntaxNode<'a>),
    ShortIntType(SyntaxNode<'a>),
    RegType(SyntaxNode<'a>),
    TimeType(SyntaxNode<'a>),
    ByteType(SyntaxNode<'a>),
    IntegerType(SyntaxNode<'a>),
}
impl<'a> IntegerType<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn signing(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn dimensions(&self) -> SyntaxList<'a, VariableDimension<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn as_long_int_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LongIntType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_int_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::IntType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logic_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bit_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::BitType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_short_int_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ShortIntType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_reg_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::RegType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_time_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::TimeType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_byte_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ByteType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_integer_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::IntegerType(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for IntegerType<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::LongIntType(node) => *node,
            Self::IntType(node) => *node,
            Self::LogicType(node) => *node,
            Self::BitType(node) => *node,
            Self::ShortIntType(node) => *node,
            Self::RegType(node) => *node,
            Self::TimeType(node) => *node,
            Self::ByteType(node) => *node,
            Self::IntegerType(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LONG_INT_TYPE
            || kind == SyntaxKind::INT_TYPE
            || kind == SyntaxKind::LOGIC_TYPE
            || kind == SyntaxKind::BIT_TYPE
            || kind == SyntaxKind::SHORT_INT_TYPE
            || kind == SyntaxKind::REG_TYPE
            || kind == SyntaxKind::TIME_TYPE
            || kind == SyntaxKind::BYTE_TYPE
            || kind == SyntaxKind::INTEGER_TYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::LONG_INT_TYPE => Some(Self::LongIntType(syntax)),
            SyntaxKind::INT_TYPE => Some(Self::IntType(syntax)),
            SyntaxKind::LOGIC_TYPE => Some(Self::LogicType(syntax)),
            SyntaxKind::BIT_TYPE => Some(Self::BitType(syntax)),
            SyntaxKind::SHORT_INT_TYPE => Some(Self::ShortIntType(syntax)),
            SyntaxKind::REG_TYPE => Some(Self::RegType(syntax)),
            SyntaxKind::TIME_TYPE => Some(Self::TimeType(syntax)),
            SyntaxKind::BYTE_TYPE => Some(Self::ByteType(syntax)),
            SyntaxKind::INTEGER_TYPE => Some(Self::IntegerType(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WildcardDimensionSpecifier<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WildcardDimensionSpecifier<'a> {
    #[inline]
    pub fn star(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for WildcardDimensionSpecifier<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WILDCARD_DIMENSION_SPECIFIER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EmptyStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn semicolon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for EmptyStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DotMemberClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DotMemberClause<'a> {
    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn member(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for DotMemberClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DOT_MEMBER_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionPattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExpressionPattern<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ExpressionPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPRESSION_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsIfElse<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RsIfElse<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn condition(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn if_item(&self) -> RsProdItem<'a> {
        self.syntax().child_node(4usize).and_then(RsProdItem::cast).unwrap()
    }

    #[inline]
    pub fn else_clause(&self) -> Option<RsElseClause<'a>> {
        self.syntax().child_node(5usize).and_then(RsElseClause::cast)
    }
}
impl<'a> AstNode<'a> for RsIfElse<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_IF_ELSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceName<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> InstanceName<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn dimensions(&self) -> SyntaxList<'a, VariableDimension<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for InstanceName<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INSTANCE_NAME
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModportPort<'a> {
    ModportNamedPort(ModportNamedPort<'a>),
    ModportSubroutinePort(ModportSubroutinePort<'a>),
    ModportExplicitPort(ModportExplicitPort<'a>),
}
impl<'a> ModportPort<'a> {
    #[inline]
    pub fn as_modport_named_port(self) -> Option<ModportNamedPort<'a>> {
        match self {
            Self::ModportNamedPort(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_modport_subroutine_port(self) -> Option<ModportSubroutinePort<'a>> {
        match self {
            Self::ModportSubroutinePort(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_modport_explicit_port(self) -> Option<ModportExplicitPort<'a>> {
        match self {
            Self::ModportExplicitPort(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ModportPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ModportNamedPort(node) => node.syntax(),
            Self::ModportSubroutinePort(node) => node.syntax(),
            Self::ModportExplicitPort(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_NAMED_PORT
            || kind == SyntaxKind::MODPORT_SUBROUTINE_PORT
            || kind == SyntaxKind::MODPORT_EXPLICIT_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::MODPORT_NAMED_PORT => {
                Some(Self::ModportNamedPort(ModportNamedPort::cast(syntax).unwrap()))
            }
            SyntaxKind::MODPORT_SUBROUTINE_PORT => {
                Some(Self::ModportSubroutinePort(ModportSubroutinePort::cast(syntax).unwrap()))
            }
            SyntaxKind::MODPORT_EXPLICIT_PORT => {
                Some(Self::ModportExplicitPort(ModportExplicitPort::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModportItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ModportItem<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ports(&self) -> AnsiPortList<'a> {
        self.syntax().child_node(1usize).and_then(AnsiPortList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ModportItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultSkewItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultSkewItem<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn direction(&self) -> ClockingDirection<'a> {
        self.syntax().child_node(2usize).and_then(ClockingDirection::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for DefaultSkewItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_SKEW_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElseConstraintClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ElseConstraintClause<'a> {
    #[inline]
    pub fn else_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn constraints(&self) -> ConstraintItem<'a> {
        self.syntax().child_node(1usize).and_then(ConstraintItem::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ElseConstraintClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ELSE_CONSTRAINT_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SolveBeforeConstraint<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SolveBeforeConstraint<'a> {
    #[inline]
    pub fn solve(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn before_expr(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn before(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn after_expr(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for SolveBeforeConstraint<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SOLVE_BEFORE_CONSTRAINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinaryBlockEventExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BinaryBlockEventExpression<'a> {
    #[inline]
    pub fn left(&self) -> BlockEventExpression<'a> {
        self.syntax().child_node(0usize).and_then(BlockEventExpression::cast).unwrap()
    }

    #[inline]
    pub fn or_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> BlockEventExpression<'a> {
        self.syntax().child_node(2usize).and_then(BlockEventExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for BinaryBlockEventExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BINARY_BLOCK_EVENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementSelectExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ElementSelectExpression<'a> {
    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn select(&self) -> ElementSelect<'a> {
        self.syntax().child_node(1usize).and_then(ElementSelect::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ElementSelectExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ELEMENT_SELECT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedType<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NamedType<'a> {
    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(0usize).and_then(Name::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for NamedType<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_TYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PatternCaseItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PatternCaseItem<'a> {
    #[inline]
    pub fn pattern(&self) -> Pattern<'a> {
        self.syntax().child_node(0usize).and_then(Pattern::cast).unwrap()
    }

    #[inline]
    pub fn triple_and(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(2usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(4usize).and_then(Statement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for PatternCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PATTERN_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReplicatedAssignmentPattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ReplicatedAssignmentPattern<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn count_expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn inner_open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn inner_close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for ReplicatedAssignmentPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::REPLICATED_ASSIGNMENT_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyPortConnection<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EmptyPortConnection<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn placeholder(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for EmptyPortConnection<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_PORT_CONNECTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisableForkStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DisableForkStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn disable(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn fork(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for DisableForkStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DISABLE_FORK_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsWeightClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RsWeightClause<'a> {
    #[inline]
    pub fn colon_equal(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn weight(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn code_block(&self) -> Option<RsProd<'a>> {
        self.syntax().child_node(2usize).and_then(RsProd::cast)
    }
}
impl<'a> AstNode<'a> for RsWeightClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_WEIGHT_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultFunctionPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultFunctionPort<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for DefaultFunctionPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_FUNCTION_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UdpPortDecl<'a> {
    UdpOutputPortDecl(UdpOutputPortDecl<'a>),
    UdpInputPortDecl(UdpInputPortDecl<'a>),
}
impl<'a> UdpPortDecl<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn as_udp_output_port_decl(self) -> Option<UdpOutputPortDecl<'a>> {
        match self {
            Self::UdpOutputPortDecl(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_udp_input_port_decl(self) -> Option<UdpInputPortDecl<'a>> {
        match self {
            Self::UdpInputPortDecl(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for UdpPortDecl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::UdpOutputPortDecl(node) => node.syntax(),
            Self::UdpInputPortDecl(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_OUTPUT_PORT_DECL || kind == SyntaxKind::UDP_INPUT_PORT_DECL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::UDP_OUTPUT_PORT_DECL => {
                Some(Self::UdpOutputPortDecl(UdpOutputPortDecl::cast(syntax).unwrap()))
            }
            SyntaxKind::UDP_INPUT_PORT_DECL => {
                Some(Self::UdpInputPortDecl(UdpInputPortDecl::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructurePattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StructurePattern<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn members(&self) -> SeparatedList<'a, StructurePatternMember<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for StructurePattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STRUCTURE_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntegerVectorExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IntegerVectorExpression<'a> {
    #[inline]
    pub fn size(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn base(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn value(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for IntegerVectorExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INTEGER_VECTOR_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LoopStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LoopStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn repeat_or_while(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(4usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(6usize).and_then(Statement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for LoopStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LOOP_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EdgeDescriptor<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EdgeDescriptor<'a> {
    #[inline]
    pub fn t_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn t_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for EdgeDescriptor<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EDGE_DESCRIPTOR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenthesizedPropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenthesizedPropertyExpr<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(1usize).and_then(PropertyExpr::cast).unwrap()
    }

    #[inline]
    pub fn match_list(&self) -> Option<SequenceMatchList<'a>> {
        self.syntax().child_node(2usize).and_then(SequenceMatchList::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ParenthesizedPropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedConditionalDirectiveExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NamedConditionalDirectiveExpression<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for NamedConditionalDirectiveExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_CONDITIONAL_DIRECTIVE_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConditionalBranchDirective<'a> {
    ElsIfDirective(SyntaxNode<'a>),
    IfDefDirective(SyntaxNode<'a>),
    IfNDefDirective(SyntaxNode<'a>),
}
impl<'a> ConditionalBranchDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> ConditionalDirectiveExpression<'a> {
        self.syntax().child_node(1usize).and_then(ConditionalDirectiveExpression::cast).unwrap()
    }

    #[inline]
    pub fn disabled_tokens(&self) -> TokenList<'a> {
        self.syntax().child_node(2usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn as_els_if_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ElsIfDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_if_def_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::IfDefDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_if_n_def_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::IfNDefDirective(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ConditionalBranchDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ElsIfDirective(node) => *node,
            Self::IfDefDirective(node) => *node,
            Self::IfNDefDirective(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ELS_IF_DIRECTIVE
            || kind == SyntaxKind::IF_DEF_DIRECTIVE
            || kind == SyntaxKind::IF_N_DEF_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ELS_IF_DIRECTIVE => Some(Self::ElsIfDirective(syntax)),
            SyntaxKind::IF_DEF_DIRECTIVE => Some(Self::IfDefDirective(syntax)),
            SyntaxKind::IF_N_DEF_DIRECTIVE => Some(Self::IfNDefDirective(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacroActualArgumentList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MacroActualArgumentList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn args(&self) -> SeparatedList<'a, MacroActualArgument<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for MacroActualArgumentList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MACRO_ACTUAL_ARGUMENT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DimensionSpecifier<'a> {
    WildcardDimensionSpecifier(WildcardDimensionSpecifier<'a>),
    RangeDimensionSpecifier(RangeDimensionSpecifier<'a>),
    QueueDimensionSpecifier(QueueDimensionSpecifier<'a>),
}
impl<'a> DimensionSpecifier<'a> {
    #[inline]
    pub fn as_wildcard_dimension_specifier(self) -> Option<WildcardDimensionSpecifier<'a>> {
        match self {
            Self::WildcardDimensionSpecifier(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_range_dimension_specifier(self) -> Option<RangeDimensionSpecifier<'a>> {
        match self {
            Self::RangeDimensionSpecifier(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_queue_dimension_specifier(self) -> Option<QueueDimensionSpecifier<'a>> {
        match self {
            Self::QueueDimensionSpecifier(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for DimensionSpecifier<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::WildcardDimensionSpecifier(node) => node.syntax(),
            Self::RangeDimensionSpecifier(node) => node.syntax(),
            Self::QueueDimensionSpecifier(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WILDCARD_DIMENSION_SPECIFIER
            || kind == SyntaxKind::RANGE_DIMENSION_SPECIFIER
            || kind == SyntaxKind::QUEUE_DIMENSION_SPECIFIER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::WILDCARD_DIMENSION_SPECIFIER => Some(Self::WildcardDimensionSpecifier(
                WildcardDimensionSpecifier::cast(syntax).unwrap(),
            )),
            SyntaxKind::RANGE_DIMENSION_SPECIFIER => {
                Some(Self::RangeDimensionSpecifier(RangeDimensionSpecifier::cast(syntax).unwrap()))
            }
            SyntaxKind::QUEUE_DIMENSION_SPECIFIER => {
                Some(Self::QueueDimensionSpecifier(QueueDimensionSpecifier::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SignedCastExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SignedCastExpression<'a> {
    #[inline]
    pub fn signing(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn apostrophe(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn inner(&self) -> ParenthesizedExpression<'a> {
        self.syntax().child_node(2usize).and_then(ParenthesizedExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for SignedCastExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIGNED_CAST_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LibraryDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LibraryDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn library(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn file_paths(&self) -> SeparatedList<'a, FilePathSpec<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn inc_dir_clause(&self) -> Option<LibraryIncDirClause<'a>> {
        self.syntax().child_node(4usize).and_then(LibraryIncDirClause::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for LibraryDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LIBRARY_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageExportAllDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PackageExportAllDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn star_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn double_colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn star_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for PackageExportAllDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PACKAGE_EXPORT_ALL_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NonAnsiPort<'a> {
    EmptyNonAnsiPort(EmptyNonAnsiPort<'a>),
    ImplicitNonAnsiPort(ImplicitNonAnsiPort<'a>),
    ExplicitNonAnsiPort(ExplicitNonAnsiPort<'a>),
}
impl<'a> NonAnsiPort<'a> {
    #[inline]
    pub fn as_empty_non_ansi_port(self) -> Option<EmptyNonAnsiPort<'a>> {
        match self {
            Self::EmptyNonAnsiPort(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_implicit_non_ansi_port(self) -> Option<ImplicitNonAnsiPort<'a>> {
        match self {
            Self::ImplicitNonAnsiPort(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_explicit_non_ansi_port(self) -> Option<ExplicitNonAnsiPort<'a>> {
        match self {
            Self::ExplicitNonAnsiPort(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for NonAnsiPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::EmptyNonAnsiPort(node) => node.syntax(),
            Self::ImplicitNonAnsiPort(node) => node.syntax(),
            Self::ExplicitNonAnsiPort(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_NON_ANSI_PORT
            || kind == SyntaxKind::IMPLICIT_NON_ANSI_PORT
            || kind == SyntaxKind::EXPLICIT_NON_ANSI_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::EMPTY_NON_ANSI_PORT => {
                Some(Self::EmptyNonAnsiPort(EmptyNonAnsiPort::cast(syntax).unwrap()))
            }
            SyntaxKind::IMPLICIT_NON_ANSI_PORT => {
                Some(Self::ImplicitNonAnsiPort(ImplicitNonAnsiPort::cast(syntax).unwrap()))
            }
            SyntaxKind::EXPLICIT_NON_ANSI_PORT => {
                Some(Self::ExplicitNonAnsiPort(ExplicitNonAnsiPort::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConstraintDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConstraintDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn qualifiers(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn specifiers(&self) -> SyntaxList<'a, ClassSpecifier<'a>> {
        self.syntax().child_node(3usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(4usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn block(&self) -> ConstraintBlock<'a> {
        self.syntax().child_node(5usize).and_then(ConstraintBlock::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ConstraintDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONSTRAINT_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropertyDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PropertyDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn port_list(&self) -> Option<AssertionItemPortList<'a>> {
        self.syntax().child_node(3usize).and_then(AssertionItemPortList::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn variables(&self) -> SyntaxList<'a, LocalVariableDeclaration<'a>> {
        self.syntax().child_node(5usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn property_spec(&self) -> PropertySpec<'a> {
        self.syntax().child_node(6usize).and_then(PropertySpec::cast).unwrap()
    }

    #[inline]
    pub fn optional_semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }

    #[inline]
    pub fn end(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(9usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for PropertyDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PROPERTY_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EqualsValueClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EqualsValueClause<'a> {
    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for EqualsValueClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EQUALS_VALUE_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModportSubroutinePortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ModportSubroutinePortList<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn import_export(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn ports(&self) -> SeparatedList<'a, ModportPort<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ModportSubroutinePortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_SUBROUTINE_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HierarchyInstantiation<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> HierarchyInstantiation<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn type_(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn parameters(&self) -> Option<ParameterValueAssignment<'a>> {
        self.syntax().child_node(2usize).and_then(ParameterValueAssignment::cast)
    }

    #[inline]
    pub fn instances(&self) -> SeparatedList<'a, HierarchicalInstance<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for HierarchyInstantiation<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::HIERARCHY_INSTANTIATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Delay<'a> {
    CycleDelay(SyntaxNode<'a>),
    DelayControl(SyntaxNode<'a>),
}
impl<'a> Delay<'a> {
    #[inline]
    pub fn hash(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn delay_value(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn as_cycle_delay(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::CycleDelay(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_delay_control(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DelayControl(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Delay<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::CycleDelay(node) => *node,
            Self::DelayControl(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CYCLE_DELAY || kind == SyntaxKind::DELAY_CONTROL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::CYCLE_DELAY => Some(Self::CycleDelay(syntax)),
            SyntaxKind::DELAY_CONTROL => Some(Self::DelayControl(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Argument<'a> {
    OrderedArgument(OrderedArgument<'a>),
    EmptyArgument(EmptyArgument<'a>),
    NamedArgument(NamedArgument<'a>),
}
impl<'a> Argument<'a> {
    #[inline]
    pub fn as_ordered_argument(self) -> Option<OrderedArgument<'a>> {
        match self {
            Self::OrderedArgument(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_empty_argument(self) -> Option<EmptyArgument<'a>> {
        match self {
            Self::EmptyArgument(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_named_argument(self) -> Option<NamedArgument<'a>> {
        match self {
            Self::NamedArgument(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Argument<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::OrderedArgument(node) => node.syntax(),
            Self::EmptyArgument(node) => node.syntax(),
            Self::NamedArgument(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ORDERED_ARGUMENT
            || kind == SyntaxKind::EMPTY_ARGUMENT
            || kind == SyntaxKind::NAMED_ARGUMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ORDERED_ARGUMENT => {
                Some(Self::OrderedArgument(OrderedArgument::cast(syntax).unwrap()))
            }
            SyntaxKind::EMPTY_ARGUMENT => {
                Some(Self::EmptyArgument(EmptyArgument::cast(syntax).unwrap()))
            }
            SyntaxKind::NAMED_ARGUMENT => {
                Some(Self::NamedArgument(NamedArgument::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BeginKeywordsDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BeginKeywordsDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn version_specifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for BeginKeywordsDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BEGIN_KEYWORDS_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConfigUseClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConfigUseClause<'a> {
    #[inline]
    pub fn use_(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<ConfigCellIdentifier<'a>> {
        self.syntax().child_node(1usize).and_then(ConfigCellIdentifier::cast)
    }

    #[inline]
    pub fn param_assignments(&self) -> Option<ParameterValueAssignment<'a>> {
        self.syntax().child_node(2usize).and_then(ParameterValueAssignment::cast)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn config(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for ConfigUseClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONFIG_USE_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CheckerInstanceStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CheckerInstanceStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn instance(&self) -> CheckerInstantiation<'a> {
        self.syntax().child_node(2usize).and_then(CheckerInstantiation::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for CheckerInstanceStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CHECKER_INSTANCE_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockEventExpression<'a> {
    PrimaryBlockEventExpression(PrimaryBlockEventExpression<'a>),
    BinaryBlockEventExpression(BinaryBlockEventExpression<'a>),
}
impl<'a> BlockEventExpression<'a> {
    #[inline]
    pub fn as_primary_block_event_expression(self) -> Option<PrimaryBlockEventExpression<'a>> {
        match self {
            Self::PrimaryBlockEventExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_block_event_expression(self) -> Option<BinaryBlockEventExpression<'a>> {
        match self {
            Self::BinaryBlockEventExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for BlockEventExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::PrimaryBlockEventExpression(node) => node.syntax(),
            Self::BinaryBlockEventExpression(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PRIMARY_BLOCK_EVENT_EXPRESSION
            || kind == SyntaxKind::BINARY_BLOCK_EVENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PRIMARY_BLOCK_EVENT_EXPRESSION => Some(Self::PrimaryBlockEventExpression(
                PrimaryBlockEventExpression::cast(syntax).unwrap(),
            )),
            SyntaxKind::BINARY_BLOCK_EVENT_EXPRESSION => Some(Self::BinaryBlockEventExpression(
                BinaryBlockEventExpression::cast(syntax).unwrap(),
            )),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransRange<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TransRange<'a> {
    #[inline]
    pub fn items(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(0usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn repeat(&self) -> Option<TransRepeatRange<'a>> {
        self.syntax().child_node(1usize).and_then(TransRepeatRange::cast)
    }
}
impl<'a> AstNode<'a> for TransRange<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TRANS_RANGE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ForeverStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ForeverStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn forever_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(3usize).and_then(Statement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ForeverStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FOREVER_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenthesizedBinsSelectExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenthesizedBinsSelectExpr<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> BinsSelectExpression<'a> {
        self.syntax().child_node(1usize).and_then(BinsSelectExpression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ParenthesizedBinsSelectExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_BINS_SELECT_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaseItem<'a> {
    DefaultCaseItem(DefaultCaseItem<'a>),
    PatternCaseItem(PatternCaseItem<'a>),
    StandardCaseItem(StandardCaseItem<'a>),
}
impl<'a> CaseItem<'a> {
    #[inline]
    pub fn as_default_case_item(self) -> Option<DefaultCaseItem<'a>> {
        match self {
            Self::DefaultCaseItem(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_pattern_case_item(self) -> Option<PatternCaseItem<'a>> {
        match self {
            Self::PatternCaseItem(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_standard_case_item(self) -> Option<StandardCaseItem<'a>> {
        match self {
            Self::StandardCaseItem(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for CaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::DefaultCaseItem(node) => node.syntax(),
            Self::PatternCaseItem(node) => node.syntax(),
            Self::StandardCaseItem(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_CASE_ITEM
            || kind == SyntaxKind::PATTERN_CASE_ITEM
            || kind == SyntaxKind::STANDARD_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::DEFAULT_CASE_ITEM => {
                Some(Self::DefaultCaseItem(DefaultCaseItem::cast(syntax).unwrap()))
            }
            SyntaxKind::PATTERN_CASE_ITEM => {
                Some(Self::PatternCaseItem(PatternCaseItem::cast(syntax).unwrap()))
            }
            SyntaxKind::STANDARD_CASE_ITEM => {
                Some(Self::StandardCaseItem(StandardCaseItem::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConfigInstanceIdentifier<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConfigInstanceIdentifier<'a> {
    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for ConfigInstanceIdentifier<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONFIG_INSTANCE_IDENTIFIER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DataType<'a> {
    IntegerType(IntegerType<'a>),
    ImplicitType(ImplicitType<'a>),
    EnumType(EnumType<'a>),
    KeywordType(KeywordType<'a>),
    NamedType(NamedType<'a>),
    VirtualInterfaceType(VirtualInterfaceType<'a>),
    TypeReference(TypeReference<'a>),
    StructUnionType(StructUnionType<'a>),
}
impl<'a> DataType<'a> {
    #[inline]
    pub fn as_integer_type(self) -> Option<IntegerType<'a>> {
        match self {
            Self::IntegerType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_implicit_type(self) -> Option<ImplicitType<'a>> {
        match self {
            Self::ImplicitType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_enum_type(self) -> Option<EnumType<'a>> {
        match self {
            Self::EnumType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_keyword_type(self) -> Option<KeywordType<'a>> {
        match self {
            Self::KeywordType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_named_type(self) -> Option<NamedType<'a>> {
        match self {
            Self::NamedType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_virtual_interface_type(self) -> Option<VirtualInterfaceType<'a>> {
        match self {
            Self::VirtualInterfaceType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_type_reference(self) -> Option<TypeReference<'a>> {
        match self {
            Self::TypeReference(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_struct_union_type(self) -> Option<StructUnionType<'a>> {
        match self {
            Self::StructUnionType(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for DataType<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::IntegerType(node) => node.syntax(),
            Self::ImplicitType(node) => node.syntax(),
            Self::EnumType(node) => node.syntax(),
            Self::KeywordType(node) => node.syntax(),
            Self::NamedType(node) => node.syntax(),
            Self::VirtualInterfaceType(node) => node.syntax(),
            Self::TypeReference(node) => node.syntax(),
            Self::StructUnionType(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BIT_TYPE
            || kind == SyntaxKind::IMPLICIT_TYPE
            || kind == SyntaxKind::LONG_INT_TYPE
            || kind == SyntaxKind::INT_TYPE
            || kind == SyntaxKind::SHORT_INT_TYPE
            || kind == SyntaxKind::TIME_TYPE
            || kind == SyntaxKind::ENUM_TYPE
            || kind == SyntaxKind::REAL_TYPE
            || kind == SyntaxKind::REG_TYPE
            || kind == SyntaxKind::NAMED_TYPE
            || kind == SyntaxKind::VIRTUAL_INTERFACE_TYPE
            || kind == SyntaxKind::INTEGER_TYPE
            || kind == SyntaxKind::REAL_TIME_TYPE
            || kind == SyntaxKind::PROPERTY_TYPE
            || kind == SyntaxKind::BYTE_TYPE
            || kind == SyntaxKind::TYPE_REFERENCE
            || kind == SyntaxKind::SHORT_REAL_TYPE
            || kind == SyntaxKind::C_HANDLE_TYPE
            || kind == SyntaxKind::STRING_TYPE
            || kind == SyntaxKind::EVENT_TYPE
            || kind == SyntaxKind::LOGIC_TYPE
            || kind == SyntaxKind::VOID_TYPE
            || kind == SyntaxKind::UNTYPED
            || kind == SyntaxKind::UNION_TYPE
            || kind == SyntaxKind::STRUCT_TYPE
            || kind == SyntaxKind::SEQUENCE_TYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::BIT_TYPE => Some(Self::IntegerType(IntegerType::cast(syntax).unwrap())),
            SyntaxKind::IMPLICIT_TYPE => {
                Some(Self::ImplicitType(ImplicitType::cast(syntax).unwrap()))
            }
            SyntaxKind::LONG_INT_TYPE => {
                Some(Self::IntegerType(IntegerType::cast(syntax).unwrap()))
            }
            SyntaxKind::INT_TYPE => Some(Self::IntegerType(IntegerType::cast(syntax).unwrap())),
            SyntaxKind::SHORT_INT_TYPE => {
                Some(Self::IntegerType(IntegerType::cast(syntax).unwrap()))
            }
            SyntaxKind::TIME_TYPE => Some(Self::IntegerType(IntegerType::cast(syntax).unwrap())),
            SyntaxKind::ENUM_TYPE => Some(Self::EnumType(EnumType::cast(syntax).unwrap())),
            SyntaxKind::REAL_TYPE => Some(Self::KeywordType(KeywordType::cast(syntax).unwrap())),
            SyntaxKind::REG_TYPE => Some(Self::IntegerType(IntegerType::cast(syntax).unwrap())),
            SyntaxKind::NAMED_TYPE => Some(Self::NamedType(NamedType::cast(syntax).unwrap())),
            SyntaxKind::VIRTUAL_INTERFACE_TYPE => {
                Some(Self::VirtualInterfaceType(VirtualInterfaceType::cast(syntax).unwrap()))
            }
            SyntaxKind::INTEGER_TYPE => Some(Self::IntegerType(IntegerType::cast(syntax).unwrap())),
            SyntaxKind::REAL_TIME_TYPE => {
                Some(Self::KeywordType(KeywordType::cast(syntax).unwrap()))
            }
            SyntaxKind::PROPERTY_TYPE => {
                Some(Self::KeywordType(KeywordType::cast(syntax).unwrap()))
            }
            SyntaxKind::BYTE_TYPE => Some(Self::IntegerType(IntegerType::cast(syntax).unwrap())),
            SyntaxKind::TYPE_REFERENCE => {
                Some(Self::TypeReference(TypeReference::cast(syntax).unwrap()))
            }
            SyntaxKind::SHORT_REAL_TYPE => {
                Some(Self::KeywordType(KeywordType::cast(syntax).unwrap()))
            }
            SyntaxKind::C_HANDLE_TYPE => {
                Some(Self::KeywordType(KeywordType::cast(syntax).unwrap()))
            }
            SyntaxKind::STRING_TYPE => Some(Self::KeywordType(KeywordType::cast(syntax).unwrap())),
            SyntaxKind::EVENT_TYPE => Some(Self::KeywordType(KeywordType::cast(syntax).unwrap())),
            SyntaxKind::LOGIC_TYPE => Some(Self::IntegerType(IntegerType::cast(syntax).unwrap())),
            SyntaxKind::VOID_TYPE => Some(Self::KeywordType(KeywordType::cast(syntax).unwrap())),
            SyntaxKind::UNTYPED => Some(Self::KeywordType(KeywordType::cast(syntax).unwrap())),
            SyntaxKind::UNION_TYPE => {
                Some(Self::StructUnionType(StructUnionType::cast(syntax).unwrap()))
            }
            SyntaxKind::STRUCT_TYPE => {
                Some(Self::StructUnionType(StructUnionType::cast(syntax).unwrap()))
            }
            SyntaxKind::SEQUENCE_TYPE => {
                Some(Self::KeywordType(KeywordType::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SequenceExpr<'a> {
    ParenthesizedSequenceExpr(ParenthesizedSequenceExpr<'a>),
    BinarySequenceExpr(BinarySequenceExpr<'a>),
    EventExpression(EventExpression<'a>),
    FirstMatchSequenceExpr(FirstMatchSequenceExpr<'a>),
    SimpleSequenceExpr(SimpleSequenceExpr<'a>),
    DelayedSequenceExpr(DelayedSequenceExpr<'a>),
    ClockingSequenceExpr(ClockingSequenceExpr<'a>),
}
impl<'a> SequenceExpr<'a> {
    #[inline]
    pub fn as_parenthesized_sequence_expr(self) -> Option<ParenthesizedSequenceExpr<'a>> {
        match self {
            Self::ParenthesizedSequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_sequence_expr(self) -> Option<BinarySequenceExpr<'a>> {
        match self {
            Self::BinarySequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_event_expression(self) -> Option<EventExpression<'a>> {
        match self {
            Self::EventExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_first_match_sequence_expr(self) -> Option<FirstMatchSequenceExpr<'a>> {
        match self {
            Self::FirstMatchSequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_simple_sequence_expr(self) -> Option<SimpleSequenceExpr<'a>> {
        match self {
            Self::SimpleSequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_delayed_sequence_expr(self) -> Option<DelayedSequenceExpr<'a>> {
        match self {
            Self::DelayedSequenceExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_clocking_sequence_expr(self) -> Option<ClockingSequenceExpr<'a>> {
        match self {
            Self::ClockingSequenceExpr(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for SequenceExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ParenthesizedSequenceExpr(node) => node.syntax(),
            Self::BinarySequenceExpr(node) => node.syntax(),
            Self::EventExpression(node) => node.syntax(),
            Self::FirstMatchSequenceExpr(node) => node.syntax(),
            Self::SimpleSequenceExpr(node) => node.syntax(),
            Self::DelayedSequenceExpr(node) => node.syntax(),
            Self::ClockingSequenceExpr(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_SEQUENCE_EXPR
            || kind == SyntaxKind::THROUGHOUT_SEQUENCE_EXPR
            || kind == SyntaxKind::INTERSECT_SEQUENCE_EXPR
            || kind == SyntaxKind::WITHIN_SEQUENCE_EXPR
            || kind == SyntaxKind::PARENTHESIZED_EVENT_EXPRESSION
            || kind == SyntaxKind::AND_SEQUENCE_EXPR
            || kind == SyntaxKind::BINARY_EVENT_EXPRESSION
            || kind == SyntaxKind::FIRST_MATCH_SEQUENCE_EXPR
            || kind == SyntaxKind::OR_SEQUENCE_EXPR
            || kind == SyntaxKind::SIMPLE_SEQUENCE_EXPR
            || kind == SyntaxKind::SIGNAL_EVENT_EXPRESSION
            || kind == SyntaxKind::DELAYED_SEQUENCE_EXPR
            || kind == SyntaxKind::CLOCKING_SEQUENCE_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PARENTHESIZED_SEQUENCE_EXPR => Some(Self::ParenthesizedSequenceExpr(
                ParenthesizedSequenceExpr::cast(syntax).unwrap(),
            )),
            SyntaxKind::THROUGHOUT_SEQUENCE_EXPR => {
                Some(Self::BinarySequenceExpr(BinarySequenceExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::INTERSECT_SEQUENCE_EXPR => {
                Some(Self::BinarySequenceExpr(BinarySequenceExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::WITHIN_SEQUENCE_EXPR => {
                Some(Self::BinarySequenceExpr(BinarySequenceExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::PARENTHESIZED_EVENT_EXPRESSION => {
                Some(Self::EventExpression(EventExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::AND_SEQUENCE_EXPR => {
                Some(Self::BinarySequenceExpr(BinarySequenceExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::BINARY_EVENT_EXPRESSION => {
                Some(Self::EventExpression(EventExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::FIRST_MATCH_SEQUENCE_EXPR => {
                Some(Self::FirstMatchSequenceExpr(FirstMatchSequenceExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::OR_SEQUENCE_EXPR => {
                Some(Self::BinarySequenceExpr(BinarySequenceExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::SIMPLE_SEQUENCE_EXPR => {
                Some(Self::SimpleSequenceExpr(SimpleSequenceExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::SIGNAL_EVENT_EXPRESSION => {
                Some(Self::EventExpression(EventExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::DELAYED_SEQUENCE_EXPR => {
                Some(Self::DelayedSequenceExpr(DelayedSequenceExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::CLOCKING_SEQUENCE_EXPR => {
                Some(Self::ClockingSequenceExpr(ClockingSequenceExpr::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PathDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn desc(&self) -> PathDescription<'a> {
        self.syntax().child_node(1usize).and_then(PathDescription::cast).unwrap()
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn delays(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(4usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }
}
impl<'a> AstNode<'a> for PathDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PATH_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModportClockingPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ModportClockingPort<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn clocking(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ModportClockingPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_CLOCKING_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransListCoverageBinInitializer<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TransListCoverageBinInitializer<'a> {
    #[inline]
    pub fn sets(&self) -> SeparatedList<'a, TransSet<'a>> {
        self.syntax().child_node(0usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for TransListCoverageBinInitializer<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TRANS_LIST_COVERAGE_BIN_INITIALIZER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinsSelectExpression<'a> {
    ParenthesizedBinsSelectExpr(ParenthesizedBinsSelectExpr<'a>),
    BinSelectWithFilterExpr(BinSelectWithFilterExpr<'a>),
    BinaryBinsSelectExpr(BinaryBinsSelectExpr<'a>),
    SimpleBinsSelectExpr(SimpleBinsSelectExpr<'a>),
    UnaryBinsSelectExpr(UnaryBinsSelectExpr<'a>),
    BinsSelectConditionExpr(BinsSelectConditionExpr<'a>),
}
impl<'a> BinsSelectExpression<'a> {
    #[inline]
    pub fn as_parenthesized_bins_select_expr(self) -> Option<ParenthesizedBinsSelectExpr<'a>> {
        match self {
            Self::ParenthesizedBinsSelectExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bin_select_with_filter_expr(self) -> Option<BinSelectWithFilterExpr<'a>> {
        match self {
            Self::BinSelectWithFilterExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_bins_select_expr(self) -> Option<BinaryBinsSelectExpr<'a>> {
        match self {
            Self::BinaryBinsSelectExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_simple_bins_select_expr(self) -> Option<SimpleBinsSelectExpr<'a>> {
        match self {
            Self::SimpleBinsSelectExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_bins_select_expr(self) -> Option<UnaryBinsSelectExpr<'a>> {
        match self {
            Self::UnaryBinsSelectExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bins_select_condition_expr(self) -> Option<BinsSelectConditionExpr<'a>> {
        match self {
            Self::BinsSelectConditionExpr(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for BinsSelectExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ParenthesizedBinsSelectExpr(node) => node.syntax(),
            Self::BinSelectWithFilterExpr(node) => node.syntax(),
            Self::BinaryBinsSelectExpr(node) => node.syntax(),
            Self::SimpleBinsSelectExpr(node) => node.syntax(),
            Self::UnaryBinsSelectExpr(node) => node.syntax(),
            Self::BinsSelectConditionExpr(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_BINS_SELECT_EXPR
            || kind == SyntaxKind::BIN_SELECT_WITH_FILTER_EXPR
            || kind == SyntaxKind::BINARY_BINS_SELECT_EXPR
            || kind == SyntaxKind::SIMPLE_BINS_SELECT_EXPR
            || kind == SyntaxKind::UNARY_BINS_SELECT_EXPR
            || kind == SyntaxKind::BINS_SELECT_CONDITION_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PARENTHESIZED_BINS_SELECT_EXPR => Some(Self::ParenthesizedBinsSelectExpr(
                ParenthesizedBinsSelectExpr::cast(syntax).unwrap(),
            )),
            SyntaxKind::BIN_SELECT_WITH_FILTER_EXPR => {
                Some(Self::BinSelectWithFilterExpr(BinSelectWithFilterExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::BINARY_BINS_SELECT_EXPR => {
                Some(Self::BinaryBinsSelectExpr(BinaryBinsSelectExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::SIMPLE_BINS_SELECT_EXPR => {
                Some(Self::SimpleBinsSelectExpr(SimpleBinsSelectExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_BINS_SELECT_EXPR => {
                Some(Self::UnaryBinsSelectExpr(UnaryBinsSelectExpr::cast(syntax).unwrap()))
            }
            SyntaxKind::BINS_SELECT_CONDITION_EXPR => {
                Some(Self::BinsSelectConditionExpr(BinsSelectConditionExpr::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModuleDeclaration<'a> {
    ModuleDeclaration(SyntaxNode<'a>),
    ProgramDeclaration(SyntaxNode<'a>),
    PackageDeclaration(SyntaxNode<'a>),
    InterfaceDeclaration(SyntaxNode<'a>),
}
impl<'a> ModuleDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn header(&self) -> ModuleHeader<'a> {
        self.syntax().child_node(1usize).and_then(ModuleHeader::cast).unwrap()
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endmodule(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(4usize).and_then(NamedBlockClause::cast)
    }

    #[inline]
    pub fn as_module_declaration(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ModuleDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_program_declaration(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ProgramDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_package_declaration(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::PackageDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_interface_declaration(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::InterfaceDeclaration(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ModuleDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ModuleDeclaration(node) => *node,
            Self::ProgramDeclaration(node) => *node,
            Self::PackageDeclaration(node) => *node,
            Self::InterfaceDeclaration(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODULE_DECLARATION
            || kind == SyntaxKind::PROGRAM_DECLARATION
            || kind == SyntaxKind::PACKAGE_DECLARATION
            || kind == SyntaxKind::INTERFACE_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::MODULE_DECLARATION => Some(Self::ModuleDeclaration(syntax)),
            SyntaxKind::PROGRAM_DECLARATION => Some(Self::ProgramDeclaration(syntax)),
            SyntaxKind::PACKAGE_DECLARATION => Some(Self::PackageDeclaration(syntax)),
            SyntaxKind::INTERFACE_DECLARATION => Some(Self::InterfaceDeclaration(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinSelectWithFilterExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BinSelectWithFilterExpr<'a> {
    #[inline]
    pub fn expr(&self) -> BinsSelectExpression<'a> {
        self.syntax().child_node(0usize).and_then(BinsSelectExpression::cast).unwrap()
    }

    #[inline]
    pub fn with(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn filter(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn matches_clause(&self) -> Option<MatchesClause<'a>> {
        self.syntax().child_node(5usize).and_then(MatchesClause::cast)
    }
}
impl<'a> AstNode<'a> for BinSelectWithFilterExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BIN_SELECT_WITH_FILTER_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimplePragmaExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SimplePragmaExpression<'a> {
    #[inline]
    pub fn value(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for SimplePragmaExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIMPLE_PRAGMA_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnaryConditionalDirectiveExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UnaryConditionalDirectiveExpression<'a> {
    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn operand(&self) -> ConditionalDirectiveExpression<'a> {
        self.syntax().child_node(1usize).and_then(ConditionalDirectiveExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for UnaryConditionalDirectiveExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNARY_CONDITIONAL_DIRECTIVE_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsElseClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RsElseClause<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn item(&self) -> RsProdItem<'a> {
        self.syntax().child_node(1usize).and_then(RsProdItem::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for RsElseClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_ELSE_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WildcardUdpPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WildcardUdpPortList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn star(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for WildcardUdpPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WILDCARD_UDP_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdentifierSelectName<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IdentifierSelectName<'a> {
    #[inline]
    pub fn identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn selectors(&self) -> SyntaxList<'a, ElementSelect<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for IdentifierSelectName<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IDENTIFIER_SELECT_NAME
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeReference<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TypeReference<'a> {
    #[inline]
    pub fn type_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for TypeReference<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TYPE_REFERENCE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransSet<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TransSet<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ranges(&self) -> SeparatedList<'a, TransRange<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for TransSet<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TRANS_SET
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeferredAssertion<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DeferredAssertion<'a> {
    #[inline]
    pub fn hash(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn zero(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn final_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for DeferredAssertion<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFERRED_ASSERTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NetDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NetDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn net_type(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn strength(&self) -> Option<NetStrength<'a>> {
        self.syntax().child_node(2usize).and_then(NetStrength::cast)
    }

    #[inline]
    pub fn expansion_hint(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(4usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn delay(&self) -> Option<TimingControl<'a>> {
        self.syntax().child_node(5usize).and_then(TimingControl::cast)
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, Declarator<'a>> {
        self.syntax().child_node(6usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }
}
impl<'a> AstNode<'a> for NetDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NET_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImmediateAssertionStatement<'a> {
    ImmediateCoverStatement(SyntaxNode<'a>),
    ImmediateAssumeStatement(SyntaxNode<'a>),
    ImmediateAssertStatement(SyntaxNode<'a>),
}
impl<'a> ImmediateAssertionStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn delay(&self) -> Option<DeferredAssertion<'a>> {
        self.syntax().child_node(3usize).and_then(DeferredAssertion::cast)
    }

    #[inline]
    pub fn expr(&self) -> ParenthesizedExpression<'a> {
        self.syntax().child_node(4usize).and_then(ParenthesizedExpression::cast).unwrap()
    }

    #[inline]
    pub fn action(&self) -> ActionBlock<'a> {
        self.syntax().child_node(5usize).and_then(ActionBlock::cast).unwrap()
    }

    #[inline]
    pub fn as_immediate_cover_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ImmediateCoverStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_immediate_assume_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ImmediateAssumeStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_immediate_assert_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ImmediateAssertStatement(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ImmediateAssertionStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ImmediateCoverStatement(node) => *node,
            Self::ImmediateAssumeStatement(node) => *node,
            Self::ImmediateAssertStatement(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMMEDIATE_COVER_STATEMENT
            || kind == SyntaxKind::IMMEDIATE_ASSUME_STATEMENT
            || kind == SyntaxKind::IMMEDIATE_ASSERT_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::IMMEDIATE_COVER_STATEMENT => Some(Self::ImmediateCoverStatement(syntax)),
            SyntaxKind::IMMEDIATE_ASSUME_STATEMENT => Some(Self::ImmediateAssumeStatement(syntax)),
            SyntaxKind::IMMEDIATE_ASSERT_STATEMENT => Some(Self::ImmediateAssertStatement(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StandardCaseItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StandardCaseItem<'a> {
    #[inline]
    pub fn expressions(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(0usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn clause(&self) -> HybridNode<'a> {
        self.syntax().child_node(2usize).and_then(HybridNode::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for StandardCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STANDARD_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultDistItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultDistItem<'a> {
    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn weight(&self) -> Option<DistWeight<'a>> {
        self.syntax().child_node(1usize).and_then(DistWeight::cast)
    }
}
impl<'a> AstNode<'a> for DefaultDistItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_DIST_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultPropertyCaseItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultPropertyCaseItem<'a> {
    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(2usize).and_then(PropertyExpr::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for DefaultPropertyCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_PROPERTY_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MatchesClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MatchesClause<'a> {
    #[inline]
    pub fn matches_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn pattern(&self) -> Pattern<'a> {
        self.syntax().child_node(1usize).and_then(Pattern::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for MatchesClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MATCHES_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Selector<'a> {
    RangeSelect(RangeSelect<'a>),
    BitSelect(BitSelect<'a>),
}
impl<'a> Selector<'a> {
    #[inline]
    pub fn as_range_select(self) -> Option<RangeSelect<'a>> {
        match self {
            Self::RangeSelect(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bit_select(self) -> Option<BitSelect<'a>> {
        match self {
            Self::BitSelect(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Selector<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::RangeSelect(node) => node.syntax(),
            Self::BitSelect(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ASCENDING_RANGE_SELECT
            || kind == SyntaxKind::DESCENDING_RANGE_SELECT
            || kind == SyntaxKind::SIMPLE_RANGE_SELECT
            || kind == SyntaxKind::BIT_SELECT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ASCENDING_RANGE_SELECT => {
                Some(Self::RangeSelect(RangeSelect::cast(syntax).unwrap()))
            }
            SyntaxKind::DESCENDING_RANGE_SELECT => {
                Some(Self::RangeSelect(RangeSelect::cast(syntax).unwrap()))
            }
            SyntaxKind::SIMPLE_RANGE_SELECT => {
                Some(Self::RangeSelect(RangeSelect::cast(syntax).unwrap()))
            }
            SyntaxKind::BIT_SELECT => Some(Self::BitSelect(BitSelect::cast(syntax).unwrap())),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProceduralAssignStatement<'a> {
    ProceduralForceStatement(SyntaxNode<'a>),
    ProceduralAssignStatement(SyntaxNode<'a>),
}
impl<'a> ProceduralAssignStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn as_procedural_force_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ProceduralForceStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_procedural_assign_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ProceduralAssignStatement(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ProceduralAssignStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ProceduralForceStatement(node) => *node,
            Self::ProceduralAssignStatement(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PROCEDURAL_FORCE_STATEMENT
            || kind == SyntaxKind::PROCEDURAL_ASSIGN_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PROCEDURAL_FORCE_STATEMENT => Some(Self::ProceduralForceStatement(syntax)),
            SyntaxKind::PROCEDURAL_ASSIGN_STATEMENT => {
                Some(Self::ProceduralAssignStatement(syntax))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PostfixUnaryExpression<'a> {
    PostincrementExpression(SyntaxNode<'a>),
    PostdecrementExpression(SyntaxNode<'a>),
}
impl<'a> PostfixUnaryExpression<'a> {
    #[inline]
    pub fn operand(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn operator_token(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn as_postincrement_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::PostincrementExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_postdecrement_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::PostdecrementExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PostfixUnaryExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::PostincrementExpression(node) => *node,
            Self::PostdecrementExpression(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::POSTINCREMENT_EXPRESSION || kind == SyntaxKind::POSTDECREMENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::POSTINCREMENT_EXPRESSION => Some(Self::PostincrementExpression(syntax)),
            SyntaxKind::POSTDECREMENT_EXPRESSION => Some(Self::PostdecrementExpression(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImplementsClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ImplementsClause<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn interfaces(&self) -> SeparatedList<'a, Name<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ImplementsClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMPLEMENTS_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UdpPortList<'a> {
    AnsiUdpPortList(AnsiUdpPortList<'a>),
    NonAnsiUdpPortList(NonAnsiUdpPortList<'a>),
    WildcardUdpPortList(WildcardUdpPortList<'a>),
}
impl<'a> UdpPortList<'a> {
    #[inline]
    pub fn as_ansi_udp_port_list(self) -> Option<AnsiUdpPortList<'a>> {
        match self {
            Self::AnsiUdpPortList(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_non_ansi_udp_port_list(self) -> Option<NonAnsiUdpPortList<'a>> {
        match self {
            Self::NonAnsiUdpPortList(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_wildcard_udp_port_list(self) -> Option<WildcardUdpPortList<'a>> {
        match self {
            Self::WildcardUdpPortList(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for UdpPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::AnsiUdpPortList(node) => node.syntax(),
            Self::NonAnsiUdpPortList(node) => node.syntax(),
            Self::WildcardUdpPortList(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ANSI_UDP_PORT_LIST
            || kind == SyntaxKind::NON_ANSI_UDP_PORT_LIST
            || kind == SyntaxKind::WILDCARD_UDP_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ANSI_UDP_PORT_LIST => {
                Some(Self::AnsiUdpPortList(AnsiUdpPortList::cast(syntax).unwrap()))
            }
            SyntaxKind::NON_ANSI_UDP_PORT_LIST => {
                Some(Self::NonAnsiUdpPortList(NonAnsiUdpPortList::cast(syntax).unwrap()))
            }
            SyntaxKind::WILDCARD_UDP_PORT_LIST => {
                Some(Self::WildcardUdpPortList(WildcardUdpPortList::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VirtualInterfaceType<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> VirtualInterfaceType<'a> {
    #[inline]
    pub fn virtual_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn interface_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn parameters(&self) -> Option<ParameterValueAssignment<'a>> {
        self.syntax().child_node(3usize).and_then(ParameterValueAssignment::cast)
    }

    #[inline]
    pub fn modport(&self) -> Option<DotMemberClause<'a>> {
        self.syntax().child_node(4usize).and_then(DotMemberClause::cast)
    }
}
impl<'a> AstNode<'a> for VirtualInterfaceType<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::VIRTUAL_INTERFACE_TYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WaitOrderStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WaitOrderStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn wait_order(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn names(&self) -> SeparatedList<'a, Name<'a>> {
        self.syntax().child_node(4usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn action(&self) -> ActionBlock<'a> {
        self.syntax().child_node(6usize).and_then(ActionBlock::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for WaitOrderStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WAIT_ORDER_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedPortConnection<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NamedPortConnection<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<PropertyExpr<'a>> {
        self.syntax().child_node(4usize).and_then(PropertyExpr::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for NamedPortConnection<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_PORT_CONNECTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyNonAnsiPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EmptyNonAnsiPort<'a> {
    #[inline]
    pub fn placeholder(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for EmptyNonAnsiPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_NON_ANSI_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventControlWithExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EventControlWithExpression<'a> {
    #[inline]
    pub fn at(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> EventExpression<'a> {
        self.syntax().child_node(1usize).and_then(EventExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for EventControlWithExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EVENT_CONTROL_WITH_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SequenceRepetition<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SequenceRepetition<'a> {
    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn selector(&self) -> Option<Selector<'a>> {
        self.syntax().child_node(2usize).and_then(Selector::cast)
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for SequenceRepetition<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SEQUENCE_REPETITION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssertionItemPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AssertionItemPortList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ports(&self) -> SeparatedList<'a, AssertionItemPort<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for AssertionItemPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ASSERTION_ITEM_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimplePathSuffix<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SimplePathSuffix<'a> {
    #[inline]
    pub fn outputs(&self) -> SeparatedList<'a, Name<'a>> {
        self.syntax().child_node(0usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for SimplePathSuffix<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIMPLE_PATH_SUFFIX
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinsSelection<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BinsSelection<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> BinsSelectExpression<'a> {
        self.syntax().child_node(4usize).and_then(BinsSelectExpression::cast).unwrap()
    }

    #[inline]
    pub fn iff(&self) -> Option<CoverageIffClause<'a>> {
        self.syntax().child_node(5usize).and_then(CoverageIffClause::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }
}
impl<'a> AstNode<'a> for BinsSelection<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BINS_SELECTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisableConstraint<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DisableConstraint<'a> {
    #[inline]
    pub fn disable(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn soft(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for DisableConstraint<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DISABLE_CONSTRAINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathDescription<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PathDescription<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn edge_identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn inputs(&self) -> SeparatedList<'a, Name<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn polarity_operator(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn path_operator(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn suffix(&self) -> PathSuffix<'a> {
        self.syntax().child_node(5usize).and_then(PathSuffix::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }
}
impl<'a> AstNode<'a> for PathDescription<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PATH_DESCRIPTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinaryBinsSelectExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BinaryBinsSelectExpr<'a> {
    #[inline]
    pub fn left(&self) -> BinsSelectExpression<'a> {
        self.syntax().child_node(0usize).and_then(BinsSelectExpression::cast).unwrap()
    }

    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> BinsSelectExpression<'a> {
        self.syntax().child_node(2usize).and_then(BinsSelectExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for BinaryBinsSelectExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BINARY_BINS_SELECT_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DelayedSequenceElement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DelayedSequenceElement<'a> {
    #[inline]
    pub fn double_hash(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn delay_val(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(1usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn range(&self) -> Option<Selector<'a>> {
        self.syntax().child_node(4usize).and_then(Selector::cast)
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn expr(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(6usize).and_then(SequenceExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for DelayedSequenceElement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DELAYED_SEQUENCE_ELEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnaryPropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UnaryPropertyExpr<'a> {
    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(1usize).and_then(PropertyExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for UnaryPropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNARY_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionalPropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConditionalPropertyExpr<'a> {
    #[inline]
    pub fn if_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn condition(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(4usize).and_then(PropertyExpr::cast).unwrap()
    }

    #[inline]
    pub fn else_clause(&self) -> Option<ElsePropertyClause<'a>> {
        self.syntax().child_node(5usize).and_then(ElsePropertyClause::cast)
    }
}
impl<'a> AstNode<'a> for ConditionalPropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONDITIONAL_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VoidCastedCallStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> VoidCastedCallStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn void_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn apostrophe(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(5usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }
}
impl<'a> AstNode<'a> for VoidCastedCallStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::VOID_CASTED_CALL_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsRule<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RsRule<'a> {
    #[inline]
    pub fn rand_join(&self) -> Option<RandJoinClause<'a>> {
        self.syntax().child_node(0usize).and_then(RandJoinClause::cast)
    }

    #[inline]
    pub fn prods(&self) -> SyntaxList<'a, RsProd<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn weight_clause(&self) -> Option<RsWeightClause<'a>> {
        self.syntax().child_node(2usize).and_then(RsWeightClause::cast)
    }
}
impl<'a> AstNode<'a> for RsRule<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_RULE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellConfigRule<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CellConfigRule<'a> {
    #[inline]
    pub fn cell(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> ConfigCellIdentifier<'a> {
        self.syntax().child_node(1usize).and_then(ConfigCellIdentifier::cast).unwrap()
    }

    #[inline]
    pub fn rule_clause(&self) -> ConfigRuleClause<'a> {
        self.syntax().child_node(2usize).and_then(ConfigRuleClause::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for CellConfigRule<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CELL_CONFIG_RULE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropertySpec<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PropertySpec<'a> {
    #[inline]
    pub fn clocking(&self) -> Option<TimingControl<'a>> {
        self.syntax().child_node(0usize).and_then(TimingControl::cast)
    }

    #[inline]
    pub fn disable(&self) -> Option<DisableIff<'a>> {
        self.syntax().child_node(1usize).and_then(DisableIff::cast)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(2usize).and_then(PropertyExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for PropertySpec<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PROPERTY_SPEC
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefParam<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefParam<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn defparam(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn assignments(&self) -> SeparatedList<'a, DefParamAssignment<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for DefParam<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEF_PARAM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageExportDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PackageExportDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, PackageImportItem<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for PackageExportDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PACKAGE_EXPORT_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImplicitType<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ImplicitType<'a> {
    #[inline]
    pub fn signing(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn dimensions(&self) -> SyntaxList<'a, VariableDimension<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn placeholder(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ImplicitType<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMPLICIT_TYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CheckerDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CheckerDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn port_list(&self) -> Option<AssertionItemPortList<'a>> {
        self.syntax().child_node(3usize).and_then(AssertionItemPortList::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(5usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(7usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for CheckerDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CHECKER_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultConfigRule<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultConfigRule<'a> {
    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn liblist(&self) -> ConfigLiblist<'a> {
        self.syntax().child_node(1usize).and_then(ConfigLiblist::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for DefaultConfigRule<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_CONFIG_RULE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefineDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefineDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn formal_arguments(&self) -> Option<MacroFormalArgumentList<'a>> {
        self.syntax().child_node(2usize).and_then(MacroFormalArgumentList::cast)
    }

    #[inline]
    pub fn body(&self) -> TokenList<'a> {
        self.syntax().child_node(3usize).and_then(TokenList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for DefineDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFINE_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WildcardPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WildcardPortList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn star(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for WildcardPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WILDCARD_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortConcatenation<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PortConcatenation<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn references(&self) -> SeparatedList<'a, PortReference<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for PortConcatenation<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PORT_CONCATENATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeParameterDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TypeParameterDeclaration<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn type_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn type_restriction(&self) -> Option<ForwardTypeRestriction<'a>> {
        self.syntax().child_node(2usize).and_then(ForwardTypeRestriction::cast)
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, TypeAssignment<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for TypeParameterDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TYPE_PARAMETER_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpEntry<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UdpEntry<'a> {
    #[inline]
    pub fn inputs(&self) -> SyntaxList<'a, UdpFieldBase<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn colon_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn current(&self) -> Option<UdpFieldBase<'a>> {
        self.syntax().child_node(2usize).and_then(UdpFieldBase::cast)
    }

    #[inline]
    pub fn colon_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn next(&self) -> Option<UdpFieldBase<'a>> {
        self.syntax().child_node(4usize).and_then(UdpFieldBase::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for UdpEntry<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_ENTRY
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsRepeat<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RsRepeat<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn item(&self) -> RsProdItem<'a> {
        self.syntax().child_node(4usize).and_then(RsProdItem::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for RsRepeat<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_REPEAT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RangeList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RangeList<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn value_ranges(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for RangeList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RANGE_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReturnStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ReturnStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn return_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn return_value(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(3usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for ReturnStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RETURN_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventControl<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EventControl<'a> {
    #[inline]
    pub fn at(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn event_name(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for EventControl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EVENT_CONTROL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WaitForkStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WaitForkStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn wait(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn fork(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for WaitForkStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WAIT_FORK_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StreamingConcatenationExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StreamingConcatenationExpression<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn operator_token(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn slice_size(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(2usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn inner_open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expressions(&self) -> SeparatedList<'a, StreamExpression<'a>> {
        self.syntax().child_node(4usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn inner_close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }
}
impl<'a> AstNode<'a> for StreamingConcatenationExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STREAMING_CONCATENATION_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypedefDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TypedefDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn typedef_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(2usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn dimensions(&self) -> SyntaxList<'a, VariableDimension<'a>> {
        self.syntax().child_node(4usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for TypedefDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TYPEDEF_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EqualsAssertionArgClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EqualsAssertionArgClause<'a> {
    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(1usize).and_then(PropertyExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for EqualsAssertionArgClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EQUALS_ASSERTION_ARG_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CaseStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CaseStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn unique_or_priority(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn case_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(5usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn matches_or_inside(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, CaseItem<'a>> {
        self.syntax().child_node(8usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endcase(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(9usize)
    }
}
impl<'a> AstNode<'a> for CaseStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CASE_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UdpDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn primitive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn port_list(&self) -> UdpPortList<'a> {
        self.syntax().child_node(3usize).and_then(UdpPortList::cast).unwrap()
    }

    #[inline]
    pub fn body(&self) -> UdpBody<'a> {
        self.syntax().child_node(4usize).and_then(UdpBody::cast).unwrap()
    }

    #[inline]
    pub fn endprimitive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(6usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for UdpDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExternModuleDecl<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExternModuleDecl<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn extern_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn actual_attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn header(&self) -> ModuleHeader<'a> {
        self.syntax().child_node(3usize).and_then(ModuleHeader::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ExternModuleDecl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXTERN_MODULE_DECL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FunctionPrototype<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> FunctionPrototype<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn specifiers(&self) -> SyntaxList<'a, ClassSpecifier<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn lifetime(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn return_type(&self) -> DataType<'a> {
        self.syntax().child_node(3usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(4usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn port_list(&self) -> Option<FunctionPortList<'a>> {
        self.syntax().child_node(5usize).and_then(FunctionPortList::cast)
    }
}
impl<'a> AstNode<'a> for FunctionPrototype<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FUNCTION_PROTOTYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UdpFieldBase<'a> {
    UdpSimpleField(UdpSimpleField<'a>),
    UdpEdgeField(UdpEdgeField<'a>),
}
impl<'a> UdpFieldBase<'a> {
    #[inline]
    pub fn as_udp_simple_field(self) -> Option<UdpSimpleField<'a>> {
        match self {
            Self::UdpSimpleField(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_udp_edge_field(self) -> Option<UdpEdgeField<'a>> {
        match self {
            Self::UdpEdgeField(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for UdpFieldBase<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::UdpSimpleField(node) => node.syntax(),
            Self::UdpEdgeField(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_SIMPLE_FIELD || kind == SyntaxKind::UDP_EDGE_FIELD
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::UDP_SIMPLE_FIELD => {
                Some(Self::UdpSimpleField(UdpSimpleField::cast(syntax).unwrap()))
            }
            SyntaxKind::UDP_EDGE_FIELD => {
                Some(Self::UdpEdgeField(UdpEdgeField::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultCoverageBinInitializer<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultCoverageBinInitializer<'a> {
    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn sequence_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for DefaultCoverageBinInitializer<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_COVERAGE_BIN_INITIALIZER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisableStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DisableStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn disable(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(3usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for DisableStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DISABLE_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DividerClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DividerClause<'a> {
    #[inline]
    pub fn divide(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn value(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for DividerClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DIVIDER_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StreamExpressionWithRange<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StreamExpressionWithRange<'a> {
    #[inline]
    pub fn with_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn range(&self) -> ElementSelect<'a> {
        self.syntax().child_node(1usize).and_then(ElementSelect::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for StreamExpressionWithRange<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STREAM_EXPRESSION_WITH_RANGE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassSpecifier<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClassSpecifier<'a> {
    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for ClassSpecifier<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLASS_SPECIFIER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElseClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ElseClause<'a> {
    #[inline]
    pub fn else_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn clause(&self) -> HybridNode<'a> {
        self.syntax().child_node(1usize).and_then(HybridNode::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ElseClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ELSE_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpecparamDeclarator<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SpecparamDeclarator<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn value_1(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn comma(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn value_2(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(5usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }
}
impl<'a> AstNode<'a> for SpecparamDeclarator<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SPECPARAM_DECLARATOR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdWithExprCoverageBinInitializer<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IdWithExprCoverageBinInitializer<'a> {
    #[inline]
    pub fn id(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn with_clause(&self) -> WithClause<'a> {
        self.syntax().child_node(1usize).and_then(WithClause::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for IdWithExprCoverageBinInitializer<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ID_WITH_EXPR_COVERAGE_BIN_INITIALIZER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageBins<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CoverageBins<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn wildcard(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn size(&self) -> Option<CoverageBinsArraySize<'a>> {
        self.syntax().child_node(4usize).and_then(CoverageBinsArraySize::cast)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn initializer(&self) -> CoverageBinInitializer<'a> {
        self.syntax().child_node(6usize).and_then(CoverageBinInitializer::cast).unwrap()
    }

    #[inline]
    pub fn iff(&self) -> Option<CoverageIffClause<'a>> {
        self.syntax().child_node(7usize).and_then(CoverageIffClause::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }
}
impl<'a> AstNode<'a> for CoverageBins<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVERAGE_BINS
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropertyCaseItem<'a> {
    StandardPropertyCaseItem(StandardPropertyCaseItem<'a>),
    DefaultPropertyCaseItem(DefaultPropertyCaseItem<'a>),
}
impl<'a> PropertyCaseItem<'a> {
    #[inline]
    pub fn as_standard_property_case_item(self) -> Option<StandardPropertyCaseItem<'a>> {
        match self {
            Self::StandardPropertyCaseItem(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_property_case_item(self) -> Option<DefaultPropertyCaseItem<'a>> {
        match self {
            Self::DefaultPropertyCaseItem(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PropertyCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::StandardPropertyCaseItem(node) => node.syntax(),
            Self::DefaultPropertyCaseItem(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STANDARD_PROPERTY_CASE_ITEM
            || kind == SyntaxKind::DEFAULT_PROPERTY_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::STANDARD_PROPERTY_CASE_ITEM => Some(Self::StandardPropertyCaseItem(
                StandardPropertyCaseItem::cast(syntax).unwrap(),
            )),
            SyntaxKind::DEFAULT_PROPERTY_CASE_ITEM => {
                Some(Self::DefaultPropertyCaseItem(DefaultPropertyCaseItem::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConditionalDirectiveExpression<'a> {
    UnaryConditionalDirectiveExpression(UnaryConditionalDirectiveExpression<'a>),
    ParenthesizedConditionalDirectiveExpression(ParenthesizedConditionalDirectiveExpression<'a>),
    NamedConditionalDirectiveExpression(NamedConditionalDirectiveExpression<'a>),
    BinaryConditionalDirectiveExpression(BinaryConditionalDirectiveExpression<'a>),
}
impl<'a> ConditionalDirectiveExpression<'a> {
    #[inline]
    pub fn as_unary_conditional_directive_expression(
        self,
    ) -> Option<UnaryConditionalDirectiveExpression<'a>> {
        match self {
            Self::UnaryConditionalDirectiveExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_parenthesized_conditional_directive_expression(
        self,
    ) -> Option<ParenthesizedConditionalDirectiveExpression<'a>> {
        match self {
            Self::ParenthesizedConditionalDirectiveExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_named_conditional_directive_expression(
        self,
    ) -> Option<NamedConditionalDirectiveExpression<'a>> {
        match self {
            Self::NamedConditionalDirectiveExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_conditional_directive_expression(
        self,
    ) -> Option<BinaryConditionalDirectiveExpression<'a>> {
        match self {
            Self::BinaryConditionalDirectiveExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ConditionalDirectiveExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::UnaryConditionalDirectiveExpression(node) => node.syntax(),
            Self::ParenthesizedConditionalDirectiveExpression(node) => node.syntax(),
            Self::NamedConditionalDirectiveExpression(node) => node.syntax(),
            Self::BinaryConditionalDirectiveExpression(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNARY_CONDITIONAL_DIRECTIVE_EXPRESSION
            || kind == SyntaxKind::PARENTHESIZED_CONDITIONAL_DIRECTIVE_EXPRESSION
            || kind == SyntaxKind::NAMED_CONDITIONAL_DIRECTIVE_EXPRESSION
            || kind == SyntaxKind::BINARY_CONDITIONAL_DIRECTIVE_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::UNARY_CONDITIONAL_DIRECTIVE_EXPRESSION => {
                Some(Self::UnaryConditionalDirectiveExpression(
                    UnaryConditionalDirectiveExpression::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::PARENTHESIZED_CONDITIONAL_DIRECTIVE_EXPRESSION => {
                Some(Self::ParenthesizedConditionalDirectiveExpression(
                    ParenthesizedConditionalDirectiveExpression::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::NAMED_CONDITIONAL_DIRECTIVE_EXPRESSION => {
                Some(Self::NamedConditionalDirectiveExpression(
                    NamedConditionalDirectiveExpression::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::BINARY_CONDITIONAL_DIRECTIVE_EXPRESSION => {
                Some(Self::BinaryConditionalDirectiveExpression(
                    BinaryConditionalDirectiveExpression::cast(syntax).unwrap(),
                ))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VariableDimension<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> VariableDimension<'a> {
    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn specifier(&self) -> Option<DimensionSpecifier<'a>> {
        self.syntax().child_node(1usize).and_then(DimensionSpecifier::cast)
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for VariableDimension<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::VARIABLE_DIMENSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FilePathSpec<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> FilePathSpec<'a> {
    #[inline]
    pub fn path(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for FilePathSpec<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FILE_PATH_SPEC
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransRepeatRange<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TransRepeatRange<'a> {
    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn specifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn selector(&self) -> Option<Selector<'a>> {
        self.syntax().child_node(2usize).and_then(Selector::cast)
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for TransRepeatRange<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TRANS_REPEAT_RANGE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IfGenerate<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IfGenerate<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn condition(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn block(&self) -> Member<'a> {
        self.syntax().child_node(5usize).and_then(Member::cast).unwrap()
    }

    #[inline]
    pub fn else_clause(&self) -> Option<ElseClause<'a>> {
        self.syntax().child_node(6usize).and_then(ElseClause::cast)
    }
}
impl<'a> AstNode<'a> for IfGenerate<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IF_GENERATE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FunctionPortBase<'a> {
    FunctionPort(FunctionPort<'a>),
    DefaultFunctionPort(DefaultFunctionPort<'a>),
}
impl<'a> FunctionPortBase<'a> {
    #[inline]
    pub fn as_function_port(self) -> Option<FunctionPort<'a>> {
        match self {
            Self::FunctionPort(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_function_port(self) -> Option<DefaultFunctionPort<'a>> {
        match self {
            Self::DefaultFunctionPort(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for FunctionPortBase<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::FunctionPort(node) => node.syntax(),
            Self::DefaultFunctionPort(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FUNCTION_PORT || kind == SyntaxKind::DEFAULT_FUNCTION_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::FUNCTION_PORT => {
                Some(Self::FunctionPort(FunctionPort::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_FUNCTION_PORT => {
                Some(Self::DefaultFunctionPort(DefaultFunctionPort::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FunctionPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> FunctionPort<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn const_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn direction(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn static_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn var_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn data_type(&self) -> Option<DataType<'a>> {
        self.syntax().child_node(5usize).and_then(DataType::cast)
    }

    #[inline]
    pub fn declarator(&self) -> Declarator<'a> {
        self.syntax().child_node(6usize).and_then(Declarator::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for FunctionPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FUNCTION_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsProdItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RsProdItem<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn arg_list(&self) -> Option<ArgumentList<'a>> {
        self.syntax().child_node(1usize).and_then(ArgumentList::cast)
    }
}
impl<'a> AstNode<'a> for RsProdItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_PROD_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SignalEventExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SignalEventExpression<'a> {
    #[inline]
    pub fn edge(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn iff_clause(&self) -> Option<IffEventClause<'a>> {
        self.syntax().child_node(2usize).and_then(IffEventClause::cast)
    }
}
impl<'a> AstNode<'a> for SignalEventExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIGNAL_EVENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedArgument<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NamedArgument<'a> {
    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<PropertyExpr<'a>> {
        self.syntax().child_node(3usize).and_then(PropertyExpr::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for NamedArgument<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_ARGUMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimpleBinsSelectExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SimpleBinsSelectExpr<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn matches_clause(&self) -> Option<MatchesClause<'a>> {
        self.syntax().child_node(1usize).and_then(MatchesClause::cast)
    }
}
impl<'a> AstNode<'a> for SimpleBinsSelectExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIMPLE_BINS_SELECT_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultDecayTimeDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultDecayTimeDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn time(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for DefaultDecayTimeDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_DECAY_TIME_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultTriregStrengthDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultTriregStrengthDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn strength(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for DefaultTriregStrengthDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_TRIREG_STRENGTH_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InterfacePortHeader<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> InterfacePortHeader<'a> {
    #[inline]
    pub fn name_or_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn modport(&self) -> Option<DotMemberClause<'a>> {
        self.syntax().child_node(1usize).and_then(DotMemberClause::cast)
    }
}
impl<'a> AstNode<'a> for InterfacePortHeader<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INTERFACE_PORT_HEADER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpInitialStmt<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UdpInitialStmt<'a> {
    #[inline]
    pub fn initial(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn value(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for UdpInitialStmt<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_INITIAL_STMT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConfigLiblist<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConfigLiblist<'a> {
    #[inline]
    pub fn liblist(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn libraries(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ConfigLiblist<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONFIG_LIBLIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IfNonePathDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IfNonePathDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn path(&self) -> PathDeclaration<'a> {
        self.syntax().child_node(2usize).and_then(PathDeclaration::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for IfNonePathDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IF_NONE_PATH_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedParamAssignment<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NamedParamAssignment<'a> {
    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(3usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for NamedParamAssignment<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_PARAM_ASSIGNMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParameterDeclarationBase<'a> {
    ParameterDeclaration(ParameterDeclaration<'a>),
    TypeParameterDeclaration(TypeParameterDeclaration<'a>),
}
impl<'a> ParameterDeclarationBase<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn as_parameter_declaration(self) -> Option<ParameterDeclaration<'a>> {
        match self {
            Self::ParameterDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_type_parameter_declaration(self) -> Option<TypeParameterDeclaration<'a>> {
        match self {
            Self::TypeParameterDeclaration(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ParameterDeclarationBase<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ParameterDeclaration(node) => node.syntax(),
            Self::TypeParameterDeclaration(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARAMETER_DECLARATION || kind == SyntaxKind::TYPE_PARAMETER_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PARAMETER_DECLARATION => {
                Some(Self::ParameterDeclaration(ParameterDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::TYPE_PARAMETER_DECLARATION => Some(Self::TypeParameterDeclaration(
                TypeParameterDeclaration::cast(syntax).unwrap(),
            )),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InsideExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> InsideExpression<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn inside(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn ranges(&self) -> RangeList<'a> {
        self.syntax().child_node(2usize).and_then(RangeList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for InsideExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INSIDE_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WildcardPortConnection<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WildcardPortConnection<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn star(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for WildcardPortConnection<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WILDCARD_PORT_CONNECTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SystemTimingCheck<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SystemTimingCheck<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn args(&self) -> SeparatedList<'a, TimingCheckArg<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for SystemTimingCheck<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SYSTEM_TIMING_CHECK
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeSpec<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AttributeSpec<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn value(&self) -> Option<EqualsValueClause<'a>> {
        self.syntax().child_node(1usize).and_then(EqualsValueClause::cast)
    }
}
impl<'a> AstNode<'a> for AttributeSpec<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ATTRIBUTE_SPEC
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StandardPropertyCaseItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StandardPropertyCaseItem<'a> {
    #[inline]
    pub fn expressions(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(0usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(2usize).and_then(PropertyExpr::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for StandardPropertyCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STANDARD_PROPERTY_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClassDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn virtual_or_interface(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn class_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn final_specifier(&self) -> Option<ClassSpecifier<'a>> {
        self.syntax().child_node(3usize).and_then(ClassSpecifier::cast)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn parameters(&self) -> Option<ParameterPortList<'a>> {
        self.syntax().child_node(5usize).and_then(ParameterPortList::cast)
    }

    #[inline]
    pub fn extends_clause(&self) -> Option<ExtendsClause<'a>> {
        self.syntax().child_node(6usize).and_then(ExtendsClause::cast)
    }

    #[inline]
    pub fn implements_clause(&self) -> Option<ImplementsClause<'a>> {
        self.syntax().child_node(7usize).and_then(ImplementsClause::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(9usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end_class(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(10usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(11usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for ClassDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLASS_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefParamAssignment<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefParamAssignment<'a> {
    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(0usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn setter(&self) -> EqualsValueClause<'a> {
        self.syntax().child_node(1usize).and_then(EqualsValueClause::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for DefParamAssignment<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEF_PARAM_ASSIGNMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimingControl<'a> {
    OneStepDelay(OneStepDelay<'a>),
    Delay(Delay<'a>),
    ImplicitEventControl(ImplicitEventControl<'a>),
    Delay3(Delay3<'a>),
    EventControlWithExpression(EventControlWithExpression<'a>),
    EventControl(EventControl<'a>),
    RepeatedEventControl(RepeatedEventControl<'a>),
}
impl<'a> TimingControl<'a> {
    #[inline]
    pub fn as_one_step_delay(self) -> Option<OneStepDelay<'a>> {
        match self {
            Self::OneStepDelay(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_delay(self) -> Option<Delay<'a>> {
        match self {
            Self::Delay(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_implicit_event_control(self) -> Option<ImplicitEventControl<'a>> {
        match self {
            Self::ImplicitEventControl(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_delay_3(self) -> Option<Delay3<'a>> {
        match self {
            Self::Delay3(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_event_control_with_expression(self) -> Option<EventControlWithExpression<'a>> {
        match self {
            Self::EventControlWithExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_event_control(self) -> Option<EventControl<'a>> {
        match self {
            Self::EventControl(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_repeated_event_control(self) -> Option<RepeatedEventControl<'a>> {
        match self {
            Self::RepeatedEventControl(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for TimingControl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::OneStepDelay(node) => node.syntax(),
            Self::Delay(node) => node.syntax(),
            Self::ImplicitEventControl(node) => node.syntax(),
            Self::Delay3(node) => node.syntax(),
            Self::EventControlWithExpression(node) => node.syntax(),
            Self::EventControl(node) => node.syntax(),
            Self::RepeatedEventControl(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ONE_STEP_DELAY
            || kind == SyntaxKind::CYCLE_DELAY
            || kind == SyntaxKind::IMPLICIT_EVENT_CONTROL
            || kind == SyntaxKind::DELAY_3
            || kind == SyntaxKind::EVENT_CONTROL_WITH_EXPRESSION
            || kind == SyntaxKind::EVENT_CONTROL
            || kind == SyntaxKind::DELAY_CONTROL
            || kind == SyntaxKind::REPEATED_EVENT_CONTROL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ONE_STEP_DELAY => {
                Some(Self::OneStepDelay(OneStepDelay::cast(syntax).unwrap()))
            }
            SyntaxKind::CYCLE_DELAY => Some(Self::Delay(Delay::cast(syntax).unwrap())),
            SyntaxKind::IMPLICIT_EVENT_CONTROL => {
                Some(Self::ImplicitEventControl(ImplicitEventControl::cast(syntax).unwrap()))
            }
            SyntaxKind::DELAY_3 => Some(Self::Delay3(Delay3::cast(syntax).unwrap())),
            SyntaxKind::EVENT_CONTROL_WITH_EXPRESSION => Some(Self::EventControlWithExpression(
                EventControlWithExpression::cast(syntax).unwrap(),
            )),
            SyntaxKind::EVENT_CONTROL => {
                Some(Self::EventControl(EventControl::cast(syntax).unwrap()))
            }
            SyntaxKind::DELAY_CONTROL => Some(Self::Delay(Delay::cast(syntax).unwrap())),
            SyntaxKind::REPEATED_EVENT_CONTROL => {
                Some(Self::RepeatedEventControl(RepeatedEventControl::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RsCaseItem<'a> {
    StandardRsCaseItem(StandardRsCaseItem<'a>),
    DefaultRsCaseItem(DefaultRsCaseItem<'a>),
}
impl<'a> RsCaseItem<'a> {
    #[inline]
    pub fn as_standard_rs_case_item(self) -> Option<StandardRsCaseItem<'a>> {
        match self {
            Self::StandardRsCaseItem(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_rs_case_item(self) -> Option<DefaultRsCaseItem<'a>> {
        match self {
            Self::DefaultRsCaseItem(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for RsCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::StandardRsCaseItem(node) => node.syntax(),
            Self::DefaultRsCaseItem(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STANDARD_RS_CASE_ITEM || kind == SyntaxKind::DEFAULT_RS_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::STANDARD_RS_CASE_ITEM => {
                Some(Self::StandardRsCaseItem(StandardRsCaseItem::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_RS_CASE_ITEM => {
                Some(Self::DefaultRsCaseItem(DefaultRsCaseItem::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NetStrength<'a> {
    ChargeStrength(ChargeStrength<'a>),
    DriveStrength(DriveStrength<'a>),
    PullStrength(PullStrength<'a>),
}
impl<'a> NetStrength<'a> {
    #[inline]
    pub fn as_charge_strength(self) -> Option<ChargeStrength<'a>> {
        match self {
            Self::ChargeStrength(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_drive_strength(self) -> Option<DriveStrength<'a>> {
        match self {
            Self::DriveStrength(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_pull_strength(self) -> Option<PullStrength<'a>> {
        match self {
            Self::PullStrength(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for NetStrength<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ChargeStrength(node) => node.syntax(),
            Self::DriveStrength(node) => node.syntax(),
            Self::PullStrength(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CHARGE_STRENGTH
            || kind == SyntaxKind::DRIVE_STRENGTH
            || kind == SyntaxKind::PULL_STRENGTH
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::CHARGE_STRENGTH => {
                Some(Self::ChargeStrength(ChargeStrength::cast(syntax).unwrap()))
            }
            SyntaxKind::DRIVE_STRENGTH => {
                Some(Self::DriveStrength(DriveStrength::cast(syntax).unwrap()))
            }
            SyntaxKind::PULL_STRENGTH => {
                Some(Self::PullStrength(PullStrength::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SequenceDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SequenceDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn port_list(&self) -> Option<AssertionItemPortList<'a>> {
        self.syntax().child_node(3usize).and_then(AssertionItemPortList::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn variables(&self) -> SyntaxList<'a, LocalVariableDeclaration<'a>> {
        self.syntax().child_node(5usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn seq_expr(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(6usize).and_then(SequenceExpr::cast).unwrap()
    }

    #[inline]
    pub fn optional_semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }

    #[inline]
    pub fn end(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(9usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for SequenceDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SEQUENCE_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionalConstraint<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConditionalConstraint<'a> {
    #[inline]
    pub fn if_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn condition(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn constraints(&self) -> ConstraintItem<'a> {
        self.syntax().child_node(4usize).and_then(ConstraintItem::cast).unwrap()
    }

    #[inline]
    pub fn else_clause(&self) -> Option<ElseConstraintClause<'a>> {
        self.syntax().child_node(5usize).and_then(ElseConstraintClause::cast)
    }
}
impl<'a> AstNode<'a> for ConditionalConstraint<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONDITIONAL_CONSTRAINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacroFormalArgumentList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MacroFormalArgumentList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn args(&self) -> SeparatedList<'a, MacroFormalArgument<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for MacroFormalArgumentList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MACRO_FORMAL_ARGUMENT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacroUsage<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MacroUsage<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn args(&self) -> Option<MacroActualArgumentList<'a>> {
        self.syntax().child_node(1usize).and_then(MacroActualArgumentList::cast)
    }
}
impl<'a> AstNode<'a> for MacroUsage<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MACRO_USAGE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OrderedStructurePatternMember<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> OrderedStructurePatternMember<'a> {
    #[inline]
    pub fn pattern(&self) -> Pattern<'a> {
        self.syntax().child_node(0usize).and_then(Pattern::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for OrderedStructurePatternMember<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ORDERED_STRUCTURE_PATTERN_MEMBER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConfigCellIdentifier<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConfigCellIdentifier<'a> {
    #[inline]
    pub fn library(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn cell(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ConfigCellIdentifier<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONFIG_CELL_IDENTIFIER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParameterValueAssignment<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParameterValueAssignment<'a> {
    #[inline]
    pub fn hash(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn parameters(&self) -> SeparatedList<'a, ParamAssignment<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ParameterValueAssignment<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARAMETER_VALUE_ASSIGNMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EdgeSensitivePathSuffix<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EdgeSensitivePathSuffix<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn outputs(&self) -> SeparatedList<'a, Name<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn polarity_operator(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(4usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for EdgeSensitivePathSuffix<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EDGE_SENSITIVE_PATH_SUFFIX
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParamAssignment<'a> {
    OrderedParamAssignment(OrderedParamAssignment<'a>),
    NamedParamAssignment(NamedParamAssignment<'a>),
}
impl<'a> ParamAssignment<'a> {
    #[inline]
    pub fn as_ordered_param_assignment(self) -> Option<OrderedParamAssignment<'a>> {
        match self {
            Self::OrderedParamAssignment(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_named_param_assignment(self) -> Option<NamedParamAssignment<'a>> {
        match self {
            Self::NamedParamAssignment(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ParamAssignment<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::OrderedParamAssignment(node) => node.syntax(),
            Self::NamedParamAssignment(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ORDERED_PARAM_ASSIGNMENT || kind == SyntaxKind::NAMED_PARAM_ASSIGNMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ORDERED_PARAM_ASSIGNMENT => {
                Some(Self::OrderedParamAssignment(OrderedParamAssignment::cast(syntax).unwrap()))
            }
            SyntaxKind::NAMED_PARAM_ASSIGNMENT => {
                Some(Self::NamedParamAssignment(NamedParamAssignment::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionalPathDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConditionalPathDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn predicate(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn path(&self) -> PathDeclaration<'a> {
        self.syntax().child_node(5usize).and_then(PathDeclaration::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ConditionalPathDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONDITIONAL_PATH_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Coverpoint<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> Coverpoint<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(1usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(2usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn coverpoint(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(4usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn iff(&self) -> Option<CoverageIffClause<'a>> {
        self.syntax().child_node(5usize).and_then(CoverageIffClause::cast)
    }

    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(7usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }

    #[inline]
    pub fn empty_semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(9usize)
    }
}
impl<'a> AstNode<'a> for Coverpoint<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVERPOINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnaryBinsSelectExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UnaryBinsSelectExpr<'a> {
    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> BinsSelectConditionExpr<'a> {
        self.syntax().child_node(1usize).and_then(BinsSelectConditionExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for UnaryBinsSelectExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNARY_BINS_SELECT_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryExpression<'a> {
    GreaterThanEqualExpression(SyntaxNode<'a>),
    LogicalEquivalenceExpression(SyntaxNode<'a>),
    ModExpression(SyntaxNode<'a>),
    BinaryOrExpression(SyntaxNode<'a>),
    DivideAssignmentExpression(SyntaxNode<'a>),
    PowerExpression(SyntaxNode<'a>),
    ArithmeticShiftLeftExpression(SyntaxNode<'a>),
    CaseInequalityExpression(SyntaxNode<'a>),
    LogicalImplicationExpression(SyntaxNode<'a>),
    MultiplyAssignmentExpression(SyntaxNode<'a>),
    InequalityExpression(SyntaxNode<'a>),
    WildcardInequalityExpression(SyntaxNode<'a>),
    AddAssignmentExpression(SyntaxNode<'a>),
    SubtractExpression(SyntaxNode<'a>),
    WildcardEqualityExpression(SyntaxNode<'a>),
    LogicalShiftRightExpression(SyntaxNode<'a>),
    EqualityExpression(SyntaxNode<'a>),
    BinaryAndExpression(SyntaxNode<'a>),
    ModAssignmentExpression(SyntaxNode<'a>),
    CaseEqualityExpression(SyntaxNode<'a>),
    LogicalShiftLeftExpression(SyntaxNode<'a>),
    AssignmentExpression(SyntaxNode<'a>),
    BinaryXnorExpression(SyntaxNode<'a>),
    LessThanEqualExpression(SyntaxNode<'a>),
    LogicalRightShiftAssignmentExpression(SyntaxNode<'a>),
    ArithmeticLeftShiftAssignmentExpression(SyntaxNode<'a>),
    NonblockingAssignmentExpression(SyntaxNode<'a>),
    ArithmeticShiftRightExpression(SyntaxNode<'a>),
    DivideExpression(SyntaxNode<'a>),
    XorAssignmentExpression(SyntaxNode<'a>),
    MultiplyExpression(SyntaxNode<'a>),
    ArithmeticRightShiftAssignmentExpression(SyntaxNode<'a>),
    GreaterThanExpression(SyntaxNode<'a>),
    LogicalOrExpression(SyntaxNode<'a>),
    LogicalLeftShiftAssignmentExpression(SyntaxNode<'a>),
    SubtractAssignmentExpression(SyntaxNode<'a>),
    AddExpression(SyntaxNode<'a>),
    OrAssignmentExpression(SyntaxNode<'a>),
    LessThanExpression(SyntaxNode<'a>),
    BinaryXorExpression(SyntaxNode<'a>),
    AndAssignmentExpression(SyntaxNode<'a>),
    LogicalAndExpression(SyntaxNode<'a>),
}
impl<'a> BinaryExpression<'a> {
    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn operator_token(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(2usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn right(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn as_greater_than_equal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::GreaterThanEqualExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logical_equivalence_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicalEquivalenceExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_mod_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ModExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_or_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::BinaryOrExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_divide_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DivideAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_power_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::PowerExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_arithmetic_shift_left_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ArithmeticShiftLeftExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_case_inequality_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::CaseInequalityExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logical_implication_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicalImplicationExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_multiply_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::MultiplyAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_inequality_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::InequalityExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_wildcard_inequality_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::WildcardInequalityExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_add_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AddAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_subtract_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::SubtractExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_wildcard_equality_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::WildcardEqualityExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logical_shift_right_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicalShiftRightExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_equality_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::EqualityExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_and_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::BinaryAndExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_mod_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ModAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_case_equality_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::CaseEqualityExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logical_shift_left_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicalShiftLeftExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_xnor_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::BinaryXnorExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_less_than_equal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LessThanEqualExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logical_right_shift_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicalRightShiftAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_arithmetic_left_shift_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ArithmeticLeftShiftAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_nonblocking_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::NonblockingAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_arithmetic_shift_right_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ArithmeticShiftRightExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_divide_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DivideExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_xor_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::XorAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_multiply_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::MultiplyExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_arithmetic_right_shift_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ArithmeticRightShiftAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_greater_than_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::GreaterThanExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logical_or_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicalOrExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logical_left_shift_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicalLeftShiftAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_subtract_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::SubtractAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_add_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AddExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_or_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::OrAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_less_than_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LessThanExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_binary_xor_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::BinaryXorExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_and_assignment_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AndAssignmentExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_logical_and_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::LogicalAndExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for BinaryExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::GreaterThanEqualExpression(node) => *node,
            Self::LogicalEquivalenceExpression(node) => *node,
            Self::ModExpression(node) => *node,
            Self::BinaryOrExpression(node) => *node,
            Self::DivideAssignmentExpression(node) => *node,
            Self::PowerExpression(node) => *node,
            Self::ArithmeticShiftLeftExpression(node) => *node,
            Self::CaseInequalityExpression(node) => *node,
            Self::LogicalImplicationExpression(node) => *node,
            Self::MultiplyAssignmentExpression(node) => *node,
            Self::InequalityExpression(node) => *node,
            Self::WildcardInequalityExpression(node) => *node,
            Self::AddAssignmentExpression(node) => *node,
            Self::SubtractExpression(node) => *node,
            Self::WildcardEqualityExpression(node) => *node,
            Self::LogicalShiftRightExpression(node) => *node,
            Self::EqualityExpression(node) => *node,
            Self::BinaryAndExpression(node) => *node,
            Self::ModAssignmentExpression(node) => *node,
            Self::CaseEqualityExpression(node) => *node,
            Self::LogicalShiftLeftExpression(node) => *node,
            Self::AssignmentExpression(node) => *node,
            Self::BinaryXnorExpression(node) => *node,
            Self::LessThanEqualExpression(node) => *node,
            Self::LogicalRightShiftAssignmentExpression(node) => *node,
            Self::ArithmeticLeftShiftAssignmentExpression(node) => *node,
            Self::NonblockingAssignmentExpression(node) => *node,
            Self::ArithmeticShiftRightExpression(node) => *node,
            Self::DivideExpression(node) => *node,
            Self::XorAssignmentExpression(node) => *node,
            Self::MultiplyExpression(node) => *node,
            Self::ArithmeticRightShiftAssignmentExpression(node) => *node,
            Self::GreaterThanExpression(node) => *node,
            Self::LogicalOrExpression(node) => *node,
            Self::LogicalLeftShiftAssignmentExpression(node) => *node,
            Self::SubtractAssignmentExpression(node) => *node,
            Self::AddExpression(node) => *node,
            Self::OrAssignmentExpression(node) => *node,
            Self::LessThanExpression(node) => *node,
            Self::BinaryXorExpression(node) => *node,
            Self::AndAssignmentExpression(node) => *node,
            Self::LogicalAndExpression(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::GREATER_THAN_EQUAL_EXPRESSION
            || kind == SyntaxKind::LOGICAL_EQUIVALENCE_EXPRESSION
            || kind == SyntaxKind::MOD_EXPRESSION
            || kind == SyntaxKind::BINARY_OR_EXPRESSION
            || kind == SyntaxKind::DIVIDE_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::POWER_EXPRESSION
            || kind == SyntaxKind::ARITHMETIC_SHIFT_LEFT_EXPRESSION
            || kind == SyntaxKind::CASE_INEQUALITY_EXPRESSION
            || kind == SyntaxKind::LOGICAL_IMPLICATION_EXPRESSION
            || kind == SyntaxKind::MULTIPLY_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::INEQUALITY_EXPRESSION
            || kind == SyntaxKind::WILDCARD_INEQUALITY_EXPRESSION
            || kind == SyntaxKind::ADD_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::SUBTRACT_EXPRESSION
            || kind == SyntaxKind::WILDCARD_EQUALITY_EXPRESSION
            || kind == SyntaxKind::LOGICAL_SHIFT_RIGHT_EXPRESSION
            || kind == SyntaxKind::EQUALITY_EXPRESSION
            || kind == SyntaxKind::BINARY_AND_EXPRESSION
            || kind == SyntaxKind::MOD_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::CASE_EQUALITY_EXPRESSION
            || kind == SyntaxKind::LOGICAL_SHIFT_LEFT_EXPRESSION
            || kind == SyntaxKind::ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::BINARY_XNOR_EXPRESSION
            || kind == SyntaxKind::LESS_THAN_EQUAL_EXPRESSION
            || kind == SyntaxKind::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::ARITHMETIC_SHIFT_RIGHT_EXPRESSION
            || kind == SyntaxKind::DIVIDE_EXPRESSION
            || kind == SyntaxKind::XOR_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::MULTIPLY_EXPRESSION
            || kind == SyntaxKind::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::GREATER_THAN_EXPRESSION
            || kind == SyntaxKind::LOGICAL_OR_EXPRESSION
            || kind == SyntaxKind::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::SUBTRACT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::ADD_EXPRESSION
            || kind == SyntaxKind::OR_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::LESS_THAN_EXPRESSION
            || kind == SyntaxKind::BINARY_XOR_EXPRESSION
            || kind == SyntaxKind::AND_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::LOGICAL_AND_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::GREATER_THAN_EQUAL_EXPRESSION => {
                Some(Self::GreaterThanEqualExpression(syntax))
            }
            SyntaxKind::LOGICAL_EQUIVALENCE_EXPRESSION => {
                Some(Self::LogicalEquivalenceExpression(syntax))
            }
            SyntaxKind::MOD_EXPRESSION => Some(Self::ModExpression(syntax)),
            SyntaxKind::BINARY_OR_EXPRESSION => Some(Self::BinaryOrExpression(syntax)),
            SyntaxKind::DIVIDE_ASSIGNMENT_EXPRESSION => {
                Some(Self::DivideAssignmentExpression(syntax))
            }
            SyntaxKind::POWER_EXPRESSION => Some(Self::PowerExpression(syntax)),
            SyntaxKind::ARITHMETIC_SHIFT_LEFT_EXPRESSION => {
                Some(Self::ArithmeticShiftLeftExpression(syntax))
            }
            SyntaxKind::CASE_INEQUALITY_EXPRESSION => Some(Self::CaseInequalityExpression(syntax)),
            SyntaxKind::LOGICAL_IMPLICATION_EXPRESSION => {
                Some(Self::LogicalImplicationExpression(syntax))
            }
            SyntaxKind::MULTIPLY_ASSIGNMENT_EXPRESSION => {
                Some(Self::MultiplyAssignmentExpression(syntax))
            }
            SyntaxKind::INEQUALITY_EXPRESSION => Some(Self::InequalityExpression(syntax)),
            SyntaxKind::WILDCARD_INEQUALITY_EXPRESSION => {
                Some(Self::WildcardInequalityExpression(syntax))
            }
            SyntaxKind::ADD_ASSIGNMENT_EXPRESSION => Some(Self::AddAssignmentExpression(syntax)),
            SyntaxKind::SUBTRACT_EXPRESSION => Some(Self::SubtractExpression(syntax)),
            SyntaxKind::WILDCARD_EQUALITY_EXPRESSION => {
                Some(Self::WildcardEqualityExpression(syntax))
            }
            SyntaxKind::LOGICAL_SHIFT_RIGHT_EXPRESSION => {
                Some(Self::LogicalShiftRightExpression(syntax))
            }
            SyntaxKind::EQUALITY_EXPRESSION => Some(Self::EqualityExpression(syntax)),
            SyntaxKind::BINARY_AND_EXPRESSION => Some(Self::BinaryAndExpression(syntax)),
            SyntaxKind::MOD_ASSIGNMENT_EXPRESSION => Some(Self::ModAssignmentExpression(syntax)),
            SyntaxKind::CASE_EQUALITY_EXPRESSION => Some(Self::CaseEqualityExpression(syntax)),
            SyntaxKind::LOGICAL_SHIFT_LEFT_EXPRESSION => {
                Some(Self::LogicalShiftLeftExpression(syntax))
            }
            SyntaxKind::ASSIGNMENT_EXPRESSION => Some(Self::AssignmentExpression(syntax)),
            SyntaxKind::BINARY_XNOR_EXPRESSION => Some(Self::BinaryXnorExpression(syntax)),
            SyntaxKind::LESS_THAN_EQUAL_EXPRESSION => Some(Self::LessThanEqualExpression(syntax)),
            SyntaxKind::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION => {
                Some(Self::LogicalRightShiftAssignmentExpression(syntax))
            }
            SyntaxKind::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION => {
                Some(Self::ArithmeticLeftShiftAssignmentExpression(syntax))
            }
            SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION => {
                Some(Self::NonblockingAssignmentExpression(syntax))
            }
            SyntaxKind::ARITHMETIC_SHIFT_RIGHT_EXPRESSION => {
                Some(Self::ArithmeticShiftRightExpression(syntax))
            }
            SyntaxKind::DIVIDE_EXPRESSION => Some(Self::DivideExpression(syntax)),
            SyntaxKind::XOR_ASSIGNMENT_EXPRESSION => Some(Self::XorAssignmentExpression(syntax)),
            SyntaxKind::MULTIPLY_EXPRESSION => Some(Self::MultiplyExpression(syntax)),
            SyntaxKind::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION => {
                Some(Self::ArithmeticRightShiftAssignmentExpression(syntax))
            }
            SyntaxKind::GREATER_THAN_EXPRESSION => Some(Self::GreaterThanExpression(syntax)),
            SyntaxKind::LOGICAL_OR_EXPRESSION => Some(Self::LogicalOrExpression(syntax)),
            SyntaxKind::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION => {
                Some(Self::LogicalLeftShiftAssignmentExpression(syntax))
            }
            SyntaxKind::SUBTRACT_ASSIGNMENT_EXPRESSION => {
                Some(Self::SubtractAssignmentExpression(syntax))
            }
            SyntaxKind::ADD_EXPRESSION => Some(Self::AddExpression(syntax)),
            SyntaxKind::OR_ASSIGNMENT_EXPRESSION => Some(Self::OrAssignmentExpression(syntax)),
            SyntaxKind::LESS_THAN_EXPRESSION => Some(Self::LessThanExpression(syntax)),
            SyntaxKind::BINARY_XOR_EXPRESSION => Some(Self::BinaryXorExpression(syntax)),
            SyntaxKind::AND_ASSIGNMENT_EXPRESSION => Some(Self::AndAssignmentExpression(syntax)),
            SyntaxKind::LOGICAL_AND_EXPRESSION => Some(Self::LogicalAndExpression(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigRule<'a> {
    CellConfigRule(CellConfigRule<'a>),
    DefaultConfigRule(DefaultConfigRule<'a>),
    InstanceConfigRule(InstanceConfigRule<'a>),
}
impl<'a> ConfigRule<'a> {
    #[inline]
    pub fn as_cell_config_rule(self) -> Option<CellConfigRule<'a>> {
        match self {
            Self::CellConfigRule(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_config_rule(self) -> Option<DefaultConfigRule<'a>> {
        match self {
            Self::DefaultConfigRule(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_instance_config_rule(self) -> Option<InstanceConfigRule<'a>> {
        match self {
            Self::InstanceConfigRule(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ConfigRule<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::CellConfigRule(node) => node.syntax(),
            Self::DefaultConfigRule(node) => node.syntax(),
            Self::InstanceConfigRule(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CELL_CONFIG_RULE
            || kind == SyntaxKind::DEFAULT_CONFIG_RULE
            || kind == SyntaxKind::INSTANCE_CONFIG_RULE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::CELL_CONFIG_RULE => {
                Some(Self::CellConfigRule(CellConfigRule::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_CONFIG_RULE => {
                Some(Self::DefaultConfigRule(DefaultConfigRule::cast(syntax).unwrap()))
            }
            SyntaxKind::INSTANCE_CONFIG_RULE => {
                Some(Self::InstanceConfigRule(InstanceConfigRule::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LibraryIncDirClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LibraryIncDirClause<'a> {
    #[inline]
    pub fn minus(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn incdir(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn file_paths(&self) -> SeparatedList<'a, FilePathSpec<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for LibraryIncDirClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LIBRARY_INC_DIR_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageImportDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PackageImportDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, PackageImportItem<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for PackageImportDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PACKAGE_IMPORT_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NonAnsiUdpPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NonAnsiUdpPortList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ports(&self) -> SeparatedList<'a, IdentifierName<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for NonAnsiUdpPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NON_ANSI_UDP_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArgumentList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ArgumentList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn parameters(&self) -> SeparatedList<'a, Argument<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ArgumentList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ARGUMENT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedStructurePatternMember<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NamedStructurePatternMember<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn pattern(&self) -> Pattern<'a> {
        self.syntax().child_node(2usize).and_then(Pattern::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for NamedStructurePatternMember<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_STRUCTURE_PATTERN_MEMBER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenthesizedExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenthesizedExpression<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expression(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ParenthesizedExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SequenceMatchList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SequenceMatchList<'a> {
    #[inline]
    pub fn comma(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, PropertyExpr<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for SequenceMatchList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SEQUENCE_MATCH_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DelayedSequenceExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DelayedSequenceExpr<'a> {
    #[inline]
    pub fn first(&self) -> Option<SequenceExpr<'a>> {
        self.syntax().child_node(0usize).and_then(SequenceExpr::cast)
    }

    #[inline]
    pub fn elements(&self) -> SyntaxList<'a, DelayedSequenceElement<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for DelayedSequenceExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DELAYED_SEQUENCE_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModportExplicitPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ModportExplicitPort<'a> {
    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(3usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for ModportExplicitPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_EXPLICIT_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NetTypeDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NetTypeDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(2usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn with_function(&self) -> Option<WithFunctionClause<'a>> {
        self.syntax().child_node(4usize).and_then(WithFunctionClause::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for NetTypeDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NET_TYPE_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Member<'a> {
    ModuleDeclaration(ModuleDeclaration<'a>),
    PrimitiveInstantiation(PrimitiveInstantiation<'a>),
    ConcurrentAssertionMember(ConcurrentAssertionMember<'a>),
    ContinuousAssign(ContinuousAssign<'a>),
    ClassPropertyDeclaration(ClassPropertyDeclaration<'a>),
    BinsSelection(BinsSelection<'a>),
    SpecifyBlock(SpecifyBlock<'a>),
    LibraryIncludeStatement(LibraryIncludeStatement<'a>),
    LoopGenerate(LoopGenerate<'a>),
    ProceduralBlock(ProceduralBlock<'a>),
    GenerateRegion(GenerateRegion<'a>),
    ClassMethodPrototype(ClassMethodPrototype<'a>),
    ExternUdpDecl(ExternUdpDecl<'a>),
    DefParam(DefParam<'a>),
    DefaultSkewItem(DefaultSkewItem<'a>),
    ModportSubroutinePortList(ModportSubroutinePortList<'a>),
    IfGenerate(IfGenerate<'a>),
    ModportSimplePortList(ModportSimplePortList<'a>),
    DPIExport(DPIExport<'a>),
    LibraryDeclaration(LibraryDeclaration<'a>),
    DefaultDisableDeclaration(DefaultDisableDeclaration<'a>),
    ExplicitAnsiPort(ExplicitAnsiPort<'a>),
    TypedefDeclaration(TypedefDeclaration<'a>),
    NetTypeDeclaration(NetTypeDeclaration<'a>),
    CheckerInstantiation(CheckerInstantiation<'a>),
    ImplicitAnsiPort(ImplicitAnsiPort<'a>),
    LocalVariableDeclaration(LocalVariableDeclaration<'a>),
    DefaultClockingReference(DefaultClockingReference<'a>),
    DPIImport(DPIImport<'a>),
    PackageExportAllDeclaration(PackageExportAllDeclaration<'a>),
    SystemTimingCheck(SystemTimingCheck<'a>),
    SequenceDeclaration(SequenceDeclaration<'a>),
    DataDeclaration(DataDeclaration<'a>),
    CheckerDeclaration(CheckerDeclaration<'a>),
    NetAlias(NetAlias<'a>),
    ParameterDeclarationStatement(ParameterDeclarationStatement<'a>),
    LetDeclaration(LetDeclaration<'a>),
    ClassDeclaration(ClassDeclaration<'a>),
    ClockingDeclaration(ClockingDeclaration<'a>),
    NetDeclaration(NetDeclaration<'a>),
    IfNonePathDeclaration(IfNonePathDeclaration<'a>),
    GenerateBlock(GenerateBlock<'a>),
    ClockingItem(ClockingItem<'a>),
    CheckerDataDeclaration(CheckerDataDeclaration<'a>),
    ForwardTypedefDeclaration(ForwardTypedefDeclaration<'a>),
    FunctionDeclaration(FunctionDeclaration<'a>),
    ConstraintPrototype(ConstraintPrototype<'a>),
    PortDeclaration(PortDeclaration<'a>),
    GenvarDeclaration(GenvarDeclaration<'a>),
    ModportClockingPort(ModportClockingPort<'a>),
    UserDefinedNetDeclaration(UserDefinedNetDeclaration<'a>),
    EmptyMember(EmptyMember<'a>),
    CoverageBins(CoverageBins<'a>),
    CovergroupDeclaration(CovergroupDeclaration<'a>),
    ClassMethodDeclaration(ClassMethodDeclaration<'a>),
    PackageImportDeclaration(PackageImportDeclaration<'a>),
    ConfigDeclaration(ConfigDeclaration<'a>),
    CaseGenerate(CaseGenerate<'a>),
    CoverCross(CoverCross<'a>),
    CoverageOption(CoverageOption<'a>),
    SpecparamDeclaration(SpecparamDeclaration<'a>),
    TimeUnitsDeclaration(TimeUnitsDeclaration<'a>),
    AnonymousProgram(AnonymousProgram<'a>),
    ExternModuleDecl(ExternModuleDecl<'a>),
    ElabSystemTask(ElabSystemTask<'a>),
    BindDirective(BindDirective<'a>),
    PathDeclaration(PathDeclaration<'a>),
    HierarchyInstantiation(HierarchyInstantiation<'a>),
    ImmediateAssertionMember(ImmediateAssertionMember<'a>),
    PropertyDeclaration(PropertyDeclaration<'a>),
    PackageExportDeclaration(PackageExportDeclaration<'a>),
    PulseStyleDeclaration(PulseStyleDeclaration<'a>),
    ModportDeclaration(ModportDeclaration<'a>),
    ExternInterfaceMethod(ExternInterfaceMethod<'a>),
    ConditionalPathDeclaration(ConditionalPathDeclaration<'a>),
    Coverpoint(Coverpoint<'a>),
    UdpDeclaration(UdpDeclaration<'a>),
    ConstraintDeclaration(ConstraintDeclaration<'a>),
}
impl<'a> Member<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn as_module_declaration(self) -> Option<ModuleDeclaration<'a>> {
        match self {
            Self::ModuleDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_primitive_instantiation(self) -> Option<PrimitiveInstantiation<'a>> {
        match self {
            Self::PrimitiveInstantiation(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_concurrent_assertion_member(self) -> Option<ConcurrentAssertionMember<'a>> {
        match self {
            Self::ConcurrentAssertionMember(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_continuous_assign(self) -> Option<ContinuousAssign<'a>> {
        match self {
            Self::ContinuousAssign(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_class_property_declaration(self) -> Option<ClassPropertyDeclaration<'a>> {
        match self {
            Self::ClassPropertyDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bins_selection(self) -> Option<BinsSelection<'a>> {
        match self {
            Self::BinsSelection(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_specify_block(self) -> Option<SpecifyBlock<'a>> {
        match self {
            Self::SpecifyBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_library_include_statement(self) -> Option<LibraryIncludeStatement<'a>> {
        match self {
            Self::LibraryIncludeStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_loop_generate(self) -> Option<LoopGenerate<'a>> {
        match self {
            Self::LoopGenerate(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_procedural_block(self) -> Option<ProceduralBlock<'a>> {
        match self {
            Self::ProceduralBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_generate_region(self) -> Option<GenerateRegion<'a>> {
        match self {
            Self::GenerateRegion(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_class_method_prototype(self) -> Option<ClassMethodPrototype<'a>> {
        match self {
            Self::ClassMethodPrototype(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_extern_udp_decl(self) -> Option<ExternUdpDecl<'a>> {
        match self {
            Self::ExternUdpDecl(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_def_param(self) -> Option<DefParam<'a>> {
        match self {
            Self::DefParam(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_skew_item(self) -> Option<DefaultSkewItem<'a>> {
        match self {
            Self::DefaultSkewItem(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_modport_subroutine_port_list(self) -> Option<ModportSubroutinePortList<'a>> {
        match self {
            Self::ModportSubroutinePortList(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_if_generate(self) -> Option<IfGenerate<'a>> {
        match self {
            Self::IfGenerate(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_modport_simple_port_list(self) -> Option<ModportSimplePortList<'a>> {
        match self {
            Self::ModportSimplePortList(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_dpi_export(self) -> Option<DPIExport<'a>> {
        match self {
            Self::DPIExport(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_library_declaration(self) -> Option<LibraryDeclaration<'a>> {
        match self {
            Self::LibraryDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_disable_declaration(self) -> Option<DefaultDisableDeclaration<'a>> {
        match self {
            Self::DefaultDisableDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_explicit_ansi_port(self) -> Option<ExplicitAnsiPort<'a>> {
        match self {
            Self::ExplicitAnsiPort(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_typedef_declaration(self) -> Option<TypedefDeclaration<'a>> {
        match self {
            Self::TypedefDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_net_type_declaration(self) -> Option<NetTypeDeclaration<'a>> {
        match self {
            Self::NetTypeDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_checker_instantiation(self) -> Option<CheckerInstantiation<'a>> {
        match self {
            Self::CheckerInstantiation(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_implicit_ansi_port(self) -> Option<ImplicitAnsiPort<'a>> {
        match self {
            Self::ImplicitAnsiPort(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_local_variable_declaration(self) -> Option<LocalVariableDeclaration<'a>> {
        match self {
            Self::LocalVariableDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_clocking_reference(self) -> Option<DefaultClockingReference<'a>> {
        match self {
            Self::DefaultClockingReference(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_dpi_import(self) -> Option<DPIImport<'a>> {
        match self {
            Self::DPIImport(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_package_export_all_declaration(self) -> Option<PackageExportAllDeclaration<'a>> {
        match self {
            Self::PackageExportAllDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_system_timing_check(self) -> Option<SystemTimingCheck<'a>> {
        match self {
            Self::SystemTimingCheck(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_sequence_declaration(self) -> Option<SequenceDeclaration<'a>> {
        match self {
            Self::SequenceDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_data_declaration(self) -> Option<DataDeclaration<'a>> {
        match self {
            Self::DataDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_checker_declaration(self) -> Option<CheckerDeclaration<'a>> {
        match self {
            Self::CheckerDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_net_alias(self) -> Option<NetAlias<'a>> {
        match self {
            Self::NetAlias(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_parameter_declaration_statement(self) -> Option<ParameterDeclarationStatement<'a>> {
        match self {
            Self::ParameterDeclarationStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_let_declaration(self) -> Option<LetDeclaration<'a>> {
        match self {
            Self::LetDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_class_declaration(self) -> Option<ClassDeclaration<'a>> {
        match self {
            Self::ClassDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_clocking_declaration(self) -> Option<ClockingDeclaration<'a>> {
        match self {
            Self::ClockingDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_net_declaration(self) -> Option<NetDeclaration<'a>> {
        match self {
            Self::NetDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_if_none_path_declaration(self) -> Option<IfNonePathDeclaration<'a>> {
        match self {
            Self::IfNonePathDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_generate_block(self) -> Option<GenerateBlock<'a>> {
        match self {
            Self::GenerateBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_clocking_item(self) -> Option<ClockingItem<'a>> {
        match self {
            Self::ClockingItem(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_checker_data_declaration(self) -> Option<CheckerDataDeclaration<'a>> {
        match self {
            Self::CheckerDataDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_forward_typedef_declaration(self) -> Option<ForwardTypedefDeclaration<'a>> {
        match self {
            Self::ForwardTypedefDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_function_declaration(self) -> Option<FunctionDeclaration<'a>> {
        match self {
            Self::FunctionDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_constraint_prototype(self) -> Option<ConstraintPrototype<'a>> {
        match self {
            Self::ConstraintPrototype(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_port_declaration(self) -> Option<PortDeclaration<'a>> {
        match self {
            Self::PortDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_genvar_declaration(self) -> Option<GenvarDeclaration<'a>> {
        match self {
            Self::GenvarDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_modport_clocking_port(self) -> Option<ModportClockingPort<'a>> {
        match self {
            Self::ModportClockingPort(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_user_defined_net_declaration(self) -> Option<UserDefinedNetDeclaration<'a>> {
        match self {
            Self::UserDefinedNetDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_empty_member(self) -> Option<EmptyMember<'a>> {
        match self {
            Self::EmptyMember(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_coverage_bins(self) -> Option<CoverageBins<'a>> {
        match self {
            Self::CoverageBins(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_covergroup_declaration(self) -> Option<CovergroupDeclaration<'a>> {
        match self {
            Self::CovergroupDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_class_method_declaration(self) -> Option<ClassMethodDeclaration<'a>> {
        match self {
            Self::ClassMethodDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_package_import_declaration(self) -> Option<PackageImportDeclaration<'a>> {
        match self {
            Self::PackageImportDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_config_declaration(self) -> Option<ConfigDeclaration<'a>> {
        match self {
            Self::ConfigDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_case_generate(self) -> Option<CaseGenerate<'a>> {
        match self {
            Self::CaseGenerate(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_cover_cross(self) -> Option<CoverCross<'a>> {
        match self {
            Self::CoverCross(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_coverage_option(self) -> Option<CoverageOption<'a>> {
        match self {
            Self::CoverageOption(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_specparam_declaration(self) -> Option<SpecparamDeclaration<'a>> {
        match self {
            Self::SpecparamDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_time_units_declaration(self) -> Option<TimeUnitsDeclaration<'a>> {
        match self {
            Self::TimeUnitsDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_anonymous_program(self) -> Option<AnonymousProgram<'a>> {
        match self {
            Self::AnonymousProgram(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_extern_module_decl(self) -> Option<ExternModuleDecl<'a>> {
        match self {
            Self::ExternModuleDecl(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_elab_system_task(self) -> Option<ElabSystemTask<'a>> {
        match self {
            Self::ElabSystemTask(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bind_directive(self) -> Option<BindDirective<'a>> {
        match self {
            Self::BindDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_path_declaration(self) -> Option<PathDeclaration<'a>> {
        match self {
            Self::PathDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_hierarchy_instantiation(self) -> Option<HierarchyInstantiation<'a>> {
        match self {
            Self::HierarchyInstantiation(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_immediate_assertion_member(self) -> Option<ImmediateAssertionMember<'a>> {
        match self {
            Self::ImmediateAssertionMember(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_property_declaration(self) -> Option<PropertyDeclaration<'a>> {
        match self {
            Self::PropertyDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_package_export_declaration(self) -> Option<PackageExportDeclaration<'a>> {
        match self {
            Self::PackageExportDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_pulse_style_declaration(self) -> Option<PulseStyleDeclaration<'a>> {
        match self {
            Self::PulseStyleDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_modport_declaration(self) -> Option<ModportDeclaration<'a>> {
        match self {
            Self::ModportDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_extern_interface_method(self) -> Option<ExternInterfaceMethod<'a>> {
        match self {
            Self::ExternInterfaceMethod(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_conditional_path_declaration(self) -> Option<ConditionalPathDeclaration<'a>> {
        match self {
            Self::ConditionalPathDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_coverpoint(self) -> Option<Coverpoint<'a>> {
        match self {
            Self::Coverpoint(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_udp_declaration(self) -> Option<UdpDeclaration<'a>> {
        match self {
            Self::UdpDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_constraint_declaration(self) -> Option<ConstraintDeclaration<'a>> {
        match self {
            Self::ConstraintDeclaration(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Member<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ModuleDeclaration(node) => node.syntax(),
            Self::PrimitiveInstantiation(node) => node.syntax(),
            Self::ConcurrentAssertionMember(node) => node.syntax(),
            Self::ContinuousAssign(node) => node.syntax(),
            Self::ClassPropertyDeclaration(node) => node.syntax(),
            Self::BinsSelection(node) => node.syntax(),
            Self::SpecifyBlock(node) => node.syntax(),
            Self::LibraryIncludeStatement(node) => node.syntax(),
            Self::LoopGenerate(node) => node.syntax(),
            Self::ProceduralBlock(node) => node.syntax(),
            Self::GenerateRegion(node) => node.syntax(),
            Self::ClassMethodPrototype(node) => node.syntax(),
            Self::ExternUdpDecl(node) => node.syntax(),
            Self::DefParam(node) => node.syntax(),
            Self::DefaultSkewItem(node) => node.syntax(),
            Self::ModportSubroutinePortList(node) => node.syntax(),
            Self::IfGenerate(node) => node.syntax(),
            Self::ModportSimplePortList(node) => node.syntax(),
            Self::DPIExport(node) => node.syntax(),
            Self::LibraryDeclaration(node) => node.syntax(),
            Self::DefaultDisableDeclaration(node) => node.syntax(),
            Self::ExplicitAnsiPort(node) => node.syntax(),
            Self::TypedefDeclaration(node) => node.syntax(),
            Self::NetTypeDeclaration(node) => node.syntax(),
            Self::CheckerInstantiation(node) => node.syntax(),
            Self::ImplicitAnsiPort(node) => node.syntax(),
            Self::LocalVariableDeclaration(node) => node.syntax(),
            Self::DefaultClockingReference(node) => node.syntax(),
            Self::DPIImport(node) => node.syntax(),
            Self::PackageExportAllDeclaration(node) => node.syntax(),
            Self::SystemTimingCheck(node) => node.syntax(),
            Self::SequenceDeclaration(node) => node.syntax(),
            Self::DataDeclaration(node) => node.syntax(),
            Self::CheckerDeclaration(node) => node.syntax(),
            Self::NetAlias(node) => node.syntax(),
            Self::ParameterDeclarationStatement(node) => node.syntax(),
            Self::LetDeclaration(node) => node.syntax(),
            Self::ClassDeclaration(node) => node.syntax(),
            Self::ClockingDeclaration(node) => node.syntax(),
            Self::NetDeclaration(node) => node.syntax(),
            Self::IfNonePathDeclaration(node) => node.syntax(),
            Self::GenerateBlock(node) => node.syntax(),
            Self::ClockingItem(node) => node.syntax(),
            Self::CheckerDataDeclaration(node) => node.syntax(),
            Self::ForwardTypedefDeclaration(node) => node.syntax(),
            Self::FunctionDeclaration(node) => node.syntax(),
            Self::ConstraintPrototype(node) => node.syntax(),
            Self::PortDeclaration(node) => node.syntax(),
            Self::GenvarDeclaration(node) => node.syntax(),
            Self::ModportClockingPort(node) => node.syntax(),
            Self::UserDefinedNetDeclaration(node) => node.syntax(),
            Self::EmptyMember(node) => node.syntax(),
            Self::CoverageBins(node) => node.syntax(),
            Self::CovergroupDeclaration(node) => node.syntax(),
            Self::ClassMethodDeclaration(node) => node.syntax(),
            Self::PackageImportDeclaration(node) => node.syntax(),
            Self::ConfigDeclaration(node) => node.syntax(),
            Self::CaseGenerate(node) => node.syntax(),
            Self::CoverCross(node) => node.syntax(),
            Self::CoverageOption(node) => node.syntax(),
            Self::SpecparamDeclaration(node) => node.syntax(),
            Self::TimeUnitsDeclaration(node) => node.syntax(),
            Self::AnonymousProgram(node) => node.syntax(),
            Self::ExternModuleDecl(node) => node.syntax(),
            Self::ElabSystemTask(node) => node.syntax(),
            Self::BindDirective(node) => node.syntax(),
            Self::PathDeclaration(node) => node.syntax(),
            Self::HierarchyInstantiation(node) => node.syntax(),
            Self::ImmediateAssertionMember(node) => node.syntax(),
            Self::PropertyDeclaration(node) => node.syntax(),
            Self::PackageExportDeclaration(node) => node.syntax(),
            Self::PulseStyleDeclaration(node) => node.syntax(),
            Self::ModportDeclaration(node) => node.syntax(),
            Self::ExternInterfaceMethod(node) => node.syntax(),
            Self::ConditionalPathDeclaration(node) => node.syntax(),
            Self::Coverpoint(node) => node.syntax(),
            Self::UdpDeclaration(node) => node.syntax(),
            Self::ConstraintDeclaration(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INTERFACE_DECLARATION
            || kind == SyntaxKind::PRIMITIVE_INSTANTIATION
            || kind == SyntaxKind::CONCURRENT_ASSERTION_MEMBER
            || kind == SyntaxKind::CONTINUOUS_ASSIGN
            || kind == SyntaxKind::CLASS_PROPERTY_DECLARATION
            || kind == SyntaxKind::BINS_SELECTION
            || kind == SyntaxKind::SPECIFY_BLOCK
            || kind == SyntaxKind::LIBRARY_INCLUDE_STATEMENT
            || kind == SyntaxKind::LOOP_GENERATE
            || kind == SyntaxKind::INITIAL_BLOCK
            || kind == SyntaxKind::GENERATE_REGION
            || kind == SyntaxKind::CLASS_METHOD_PROTOTYPE
            || kind == SyntaxKind::EXTERN_UDP_DECL
            || kind == SyntaxKind::DEF_PARAM
            || kind == SyntaxKind::DEFAULT_SKEW_ITEM
            || kind == SyntaxKind::MODPORT_SUBROUTINE_PORT_LIST
            || kind == SyntaxKind::IF_GENERATE
            || kind == SyntaxKind::MODPORT_SIMPLE_PORT_LIST
            || kind == SyntaxKind::DPI_EXPORT
            || kind == SyntaxKind::LIBRARY_DECLARATION
            || kind == SyntaxKind::DEFAULT_DISABLE_DECLARATION
            || kind == SyntaxKind::EXPLICIT_ANSI_PORT
            || kind == SyntaxKind::TYPEDEF_DECLARATION
            || kind == SyntaxKind::NET_TYPE_DECLARATION
            || kind == SyntaxKind::CHECKER_INSTANTIATION
            || kind == SyntaxKind::IMPLICIT_ANSI_PORT
            || kind == SyntaxKind::LOCAL_VARIABLE_DECLARATION
            || kind == SyntaxKind::ALWAYS_BLOCK
            || kind == SyntaxKind::DEFAULT_CLOCKING_REFERENCE
            || kind == SyntaxKind::DPI_IMPORT
            || kind == SyntaxKind::PACKAGE_EXPORT_ALL_DECLARATION
            || kind == SyntaxKind::SYSTEM_TIMING_CHECK
            || kind == SyntaxKind::SEQUENCE_DECLARATION
            || kind == SyntaxKind::DATA_DECLARATION
            || kind == SyntaxKind::CHECKER_DECLARATION
            || kind == SyntaxKind::NET_ALIAS
            || kind == SyntaxKind::ALWAYS_LATCH_BLOCK
            || kind == SyntaxKind::PARAMETER_DECLARATION_STATEMENT
            || kind == SyntaxKind::LET_DECLARATION
            || kind == SyntaxKind::CLASS_DECLARATION
            || kind == SyntaxKind::CLOCKING_DECLARATION
            || kind == SyntaxKind::NET_DECLARATION
            || kind == SyntaxKind::IF_NONE_PATH_DECLARATION
            || kind == SyntaxKind::GENERATE_BLOCK
            || kind == SyntaxKind::CLOCKING_ITEM
            || kind == SyntaxKind::CHECKER_DATA_DECLARATION
            || kind == SyntaxKind::FORWARD_TYPEDEF_DECLARATION
            || kind == SyntaxKind::MODULE_DECLARATION
            || kind == SyntaxKind::TASK_DECLARATION
            || kind == SyntaxKind::ALWAYS_COMB_BLOCK
            || kind == SyntaxKind::CONSTRAINT_PROTOTYPE
            || kind == SyntaxKind::PORT_DECLARATION
            || kind == SyntaxKind::PACKAGE_DECLARATION
            || kind == SyntaxKind::GENVAR_DECLARATION
            || kind == SyntaxKind::MODPORT_CLOCKING_PORT
            || kind == SyntaxKind::USER_DEFINED_NET_DECLARATION
            || kind == SyntaxKind::FUNCTION_DECLARATION
            || kind == SyntaxKind::EMPTY_MEMBER
            || kind == SyntaxKind::COVERAGE_BINS
            || kind == SyntaxKind::COVERGROUP_DECLARATION
            || kind == SyntaxKind::CLASS_METHOD_DECLARATION
            || kind == SyntaxKind::PACKAGE_IMPORT_DECLARATION
            || kind == SyntaxKind::CONFIG_DECLARATION
            || kind == SyntaxKind::CASE_GENERATE
            || kind == SyntaxKind::COVER_CROSS
            || kind == SyntaxKind::COVERAGE_OPTION
            || kind == SyntaxKind::SPECPARAM_DECLARATION
            || kind == SyntaxKind::TIME_UNITS_DECLARATION
            || kind == SyntaxKind::ANONYMOUS_PROGRAM
            || kind == SyntaxKind::EXTERN_MODULE_DECL
            || kind == SyntaxKind::ELAB_SYSTEM_TASK
            || kind == SyntaxKind::BIND_DIRECTIVE
            || kind == SyntaxKind::PATH_DECLARATION
            || kind == SyntaxKind::HIERARCHY_INSTANTIATION
            || kind == SyntaxKind::IMMEDIATE_ASSERTION_MEMBER
            || kind == SyntaxKind::PROPERTY_DECLARATION
            || kind == SyntaxKind::PACKAGE_EXPORT_DECLARATION
            || kind == SyntaxKind::PULSE_STYLE_DECLARATION
            || kind == SyntaxKind::MODPORT_DECLARATION
            || kind == SyntaxKind::ALWAYS_FF_BLOCK
            || kind == SyntaxKind::EXTERN_INTERFACE_METHOD
            || kind == SyntaxKind::PROGRAM_DECLARATION
            || kind == SyntaxKind::CONDITIONAL_PATH_DECLARATION
            || kind == SyntaxKind::COVERPOINT
            || kind == SyntaxKind::FINAL_BLOCK
            || kind == SyntaxKind::UDP_DECLARATION
            || kind == SyntaxKind::CONSTRAINT_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::INTERFACE_DECLARATION => {
                Some(Self::ModuleDeclaration(ModuleDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::PRIMITIVE_INSTANTIATION => {
                Some(Self::PrimitiveInstantiation(PrimitiveInstantiation::cast(syntax).unwrap()))
            }
            SyntaxKind::CONCURRENT_ASSERTION_MEMBER => Some(Self::ConcurrentAssertionMember(
                ConcurrentAssertionMember::cast(syntax).unwrap(),
            )),
            SyntaxKind::CONTINUOUS_ASSIGN => {
                Some(Self::ContinuousAssign(ContinuousAssign::cast(syntax).unwrap()))
            }
            SyntaxKind::CLASS_PROPERTY_DECLARATION => Some(Self::ClassPropertyDeclaration(
                ClassPropertyDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::BINS_SELECTION => {
                Some(Self::BinsSelection(BinsSelection::cast(syntax).unwrap()))
            }
            SyntaxKind::SPECIFY_BLOCK => {
                Some(Self::SpecifyBlock(SpecifyBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::LIBRARY_INCLUDE_STATEMENT => {
                Some(Self::LibraryIncludeStatement(LibraryIncludeStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::LOOP_GENERATE => {
                Some(Self::LoopGenerate(LoopGenerate::cast(syntax).unwrap()))
            }
            SyntaxKind::INITIAL_BLOCK => {
                Some(Self::ProceduralBlock(ProceduralBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::GENERATE_REGION => {
                Some(Self::GenerateRegion(GenerateRegion::cast(syntax).unwrap()))
            }
            SyntaxKind::CLASS_METHOD_PROTOTYPE => {
                Some(Self::ClassMethodPrototype(ClassMethodPrototype::cast(syntax).unwrap()))
            }
            SyntaxKind::EXTERN_UDP_DECL => {
                Some(Self::ExternUdpDecl(ExternUdpDecl::cast(syntax).unwrap()))
            }
            SyntaxKind::DEF_PARAM => Some(Self::DefParam(DefParam::cast(syntax).unwrap())),
            SyntaxKind::DEFAULT_SKEW_ITEM => {
                Some(Self::DefaultSkewItem(DefaultSkewItem::cast(syntax).unwrap()))
            }
            SyntaxKind::MODPORT_SUBROUTINE_PORT_LIST => Some(Self::ModportSubroutinePortList(
                ModportSubroutinePortList::cast(syntax).unwrap(),
            )),
            SyntaxKind::IF_GENERATE => Some(Self::IfGenerate(IfGenerate::cast(syntax).unwrap())),
            SyntaxKind::MODPORT_SIMPLE_PORT_LIST => {
                Some(Self::ModportSimplePortList(ModportSimplePortList::cast(syntax).unwrap()))
            }
            SyntaxKind::DPI_EXPORT => Some(Self::DPIExport(DPIExport::cast(syntax).unwrap())),
            SyntaxKind::LIBRARY_DECLARATION => {
                Some(Self::LibraryDeclaration(LibraryDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_DISABLE_DECLARATION => Some(Self::DefaultDisableDeclaration(
                DefaultDisableDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::EXPLICIT_ANSI_PORT => {
                Some(Self::ExplicitAnsiPort(ExplicitAnsiPort::cast(syntax).unwrap()))
            }
            SyntaxKind::TYPEDEF_DECLARATION => {
                Some(Self::TypedefDeclaration(TypedefDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::NET_TYPE_DECLARATION => {
                Some(Self::NetTypeDeclaration(NetTypeDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::CHECKER_INSTANTIATION => {
                Some(Self::CheckerInstantiation(CheckerInstantiation::cast(syntax).unwrap()))
            }
            SyntaxKind::IMPLICIT_ANSI_PORT => {
                Some(Self::ImplicitAnsiPort(ImplicitAnsiPort::cast(syntax).unwrap()))
            }
            SyntaxKind::LOCAL_VARIABLE_DECLARATION => Some(Self::LocalVariableDeclaration(
                LocalVariableDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::ALWAYS_BLOCK => {
                Some(Self::ProceduralBlock(ProceduralBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_CLOCKING_REFERENCE => Some(Self::DefaultClockingReference(
                DefaultClockingReference::cast(syntax).unwrap(),
            )),
            SyntaxKind::DPI_IMPORT => Some(Self::DPIImport(DPIImport::cast(syntax).unwrap())),
            SyntaxKind::PACKAGE_EXPORT_ALL_DECLARATION => Some(Self::PackageExportAllDeclaration(
                PackageExportAllDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::SYSTEM_TIMING_CHECK => {
                Some(Self::SystemTimingCheck(SystemTimingCheck::cast(syntax).unwrap()))
            }
            SyntaxKind::SEQUENCE_DECLARATION => {
                Some(Self::SequenceDeclaration(SequenceDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::DATA_DECLARATION => {
                Some(Self::DataDeclaration(DataDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::CHECKER_DECLARATION => {
                Some(Self::CheckerDeclaration(CheckerDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::NET_ALIAS => Some(Self::NetAlias(NetAlias::cast(syntax).unwrap())),
            SyntaxKind::ALWAYS_LATCH_BLOCK => {
                Some(Self::ProceduralBlock(ProceduralBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::PARAMETER_DECLARATION_STATEMENT => {
                Some(Self::ParameterDeclarationStatement(
                    ParameterDeclarationStatement::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::LET_DECLARATION => {
                Some(Self::LetDeclaration(LetDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::CLASS_DECLARATION => {
                Some(Self::ClassDeclaration(ClassDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::CLOCKING_DECLARATION => {
                Some(Self::ClockingDeclaration(ClockingDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::NET_DECLARATION => {
                Some(Self::NetDeclaration(NetDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::IF_NONE_PATH_DECLARATION => {
                Some(Self::IfNonePathDeclaration(IfNonePathDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::GENERATE_BLOCK => {
                Some(Self::GenerateBlock(GenerateBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::CLOCKING_ITEM => {
                Some(Self::ClockingItem(ClockingItem::cast(syntax).unwrap()))
            }
            SyntaxKind::CHECKER_DATA_DECLARATION => {
                Some(Self::CheckerDataDeclaration(CheckerDataDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::FORWARD_TYPEDEF_DECLARATION => Some(Self::ForwardTypedefDeclaration(
                ForwardTypedefDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::MODULE_DECLARATION => {
                Some(Self::ModuleDeclaration(ModuleDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::TASK_DECLARATION => {
                Some(Self::FunctionDeclaration(FunctionDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::ALWAYS_COMB_BLOCK => {
                Some(Self::ProceduralBlock(ProceduralBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::CONSTRAINT_PROTOTYPE => {
                Some(Self::ConstraintPrototype(ConstraintPrototype::cast(syntax).unwrap()))
            }
            SyntaxKind::PORT_DECLARATION => {
                Some(Self::PortDeclaration(PortDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::PACKAGE_DECLARATION => {
                Some(Self::ModuleDeclaration(ModuleDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::GENVAR_DECLARATION => {
                Some(Self::GenvarDeclaration(GenvarDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::MODPORT_CLOCKING_PORT => {
                Some(Self::ModportClockingPort(ModportClockingPort::cast(syntax).unwrap()))
            }
            SyntaxKind::USER_DEFINED_NET_DECLARATION => Some(Self::UserDefinedNetDeclaration(
                UserDefinedNetDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::FUNCTION_DECLARATION => {
                Some(Self::FunctionDeclaration(FunctionDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::EMPTY_MEMBER => Some(Self::EmptyMember(EmptyMember::cast(syntax).unwrap())),
            SyntaxKind::COVERAGE_BINS => {
                Some(Self::CoverageBins(CoverageBins::cast(syntax).unwrap()))
            }
            SyntaxKind::COVERGROUP_DECLARATION => {
                Some(Self::CovergroupDeclaration(CovergroupDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::CLASS_METHOD_DECLARATION => {
                Some(Self::ClassMethodDeclaration(ClassMethodDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::PACKAGE_IMPORT_DECLARATION => Some(Self::PackageImportDeclaration(
                PackageImportDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::CONFIG_DECLARATION => {
                Some(Self::ConfigDeclaration(ConfigDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::CASE_GENERATE => {
                Some(Self::CaseGenerate(CaseGenerate::cast(syntax).unwrap()))
            }
            SyntaxKind::COVER_CROSS => Some(Self::CoverCross(CoverCross::cast(syntax).unwrap())),
            SyntaxKind::COVERAGE_OPTION => {
                Some(Self::CoverageOption(CoverageOption::cast(syntax).unwrap()))
            }
            SyntaxKind::SPECPARAM_DECLARATION => {
                Some(Self::SpecparamDeclaration(SpecparamDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::TIME_UNITS_DECLARATION => {
                Some(Self::TimeUnitsDeclaration(TimeUnitsDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::ANONYMOUS_PROGRAM => {
                Some(Self::AnonymousProgram(AnonymousProgram::cast(syntax).unwrap()))
            }
            SyntaxKind::EXTERN_MODULE_DECL => {
                Some(Self::ExternModuleDecl(ExternModuleDecl::cast(syntax).unwrap()))
            }
            SyntaxKind::ELAB_SYSTEM_TASK => {
                Some(Self::ElabSystemTask(ElabSystemTask::cast(syntax).unwrap()))
            }
            SyntaxKind::BIND_DIRECTIVE => {
                Some(Self::BindDirective(BindDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::PATH_DECLARATION => {
                Some(Self::PathDeclaration(PathDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::HIERARCHY_INSTANTIATION => {
                Some(Self::HierarchyInstantiation(HierarchyInstantiation::cast(syntax).unwrap()))
            }
            SyntaxKind::IMMEDIATE_ASSERTION_MEMBER => Some(Self::ImmediateAssertionMember(
                ImmediateAssertionMember::cast(syntax).unwrap(),
            )),
            SyntaxKind::PROPERTY_DECLARATION => {
                Some(Self::PropertyDeclaration(PropertyDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::PACKAGE_EXPORT_DECLARATION => Some(Self::PackageExportDeclaration(
                PackageExportDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::PULSE_STYLE_DECLARATION => {
                Some(Self::PulseStyleDeclaration(PulseStyleDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::MODPORT_DECLARATION => {
                Some(Self::ModportDeclaration(ModportDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::ALWAYS_FF_BLOCK => {
                Some(Self::ProceduralBlock(ProceduralBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::EXTERN_INTERFACE_METHOD => {
                Some(Self::ExternInterfaceMethod(ExternInterfaceMethod::cast(syntax).unwrap()))
            }
            SyntaxKind::PROGRAM_DECLARATION => {
                Some(Self::ModuleDeclaration(ModuleDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::CONDITIONAL_PATH_DECLARATION => Some(Self::ConditionalPathDeclaration(
                ConditionalPathDeclaration::cast(syntax).unwrap(),
            )),
            SyntaxKind::COVERPOINT => Some(Self::Coverpoint(Coverpoint::cast(syntax).unwrap())),
            SyntaxKind::FINAL_BLOCK => {
                Some(Self::ProceduralBlock(ProceduralBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::UDP_DECLARATION => {
                Some(Self::UdpDeclaration(UdpDeclaration::cast(syntax).unwrap()))
            }
            SyntaxKind::CONSTRAINT_DECLARATION => {
                Some(Self::ConstraintDeclaration(ConstraintDeclaration::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ForwardTypeRestriction<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ForwardTypeRestriction<'a> {
    #[inline]
    pub fn keyword_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn keyword_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for ForwardTypeRestriction<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FORWARD_TYPE_RESTRICTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LiteralExpression<'a> {
    WildcardLiteralExpression(SyntaxNode<'a>),
    RealLiteralExpression(SyntaxNode<'a>),
    UnbasedUnsizedLiteralExpression(SyntaxNode<'a>),
    NullLiteralExpression(SyntaxNode<'a>),
    DefaultPatternKeyExpression(SyntaxNode<'a>),
    StringLiteralExpression(SyntaxNode<'a>),
    TimeLiteralExpression(SyntaxNode<'a>),
    IntegerLiteralExpression(SyntaxNode<'a>),
}
impl<'a> LiteralExpression<'a> {
    #[inline]
    pub fn literal(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn as_wildcard_literal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::WildcardLiteralExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_real_literal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::RealLiteralExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unbased_unsized_literal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnbasedUnsizedLiteralExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_null_literal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::NullLiteralExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_pattern_key_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DefaultPatternKeyExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_string_literal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::StringLiteralExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_time_literal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::TimeLiteralExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_integer_literal_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::IntegerLiteralExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for LiteralExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::WildcardLiteralExpression(node) => *node,
            Self::RealLiteralExpression(node) => *node,
            Self::UnbasedUnsizedLiteralExpression(node) => *node,
            Self::NullLiteralExpression(node) => *node,
            Self::DefaultPatternKeyExpression(node) => *node,
            Self::StringLiteralExpression(node) => *node,
            Self::TimeLiteralExpression(node) => *node,
            Self::IntegerLiteralExpression(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WILDCARD_LITERAL_EXPRESSION
            || kind == SyntaxKind::REAL_LITERAL_EXPRESSION
            || kind == SyntaxKind::UNBASED_UNSIZED_LITERAL_EXPRESSION
            || kind == SyntaxKind::NULL_LITERAL_EXPRESSION
            || kind == SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION
            || kind == SyntaxKind::STRING_LITERAL_EXPRESSION
            || kind == SyntaxKind::TIME_LITERAL_EXPRESSION
            || kind == SyntaxKind::INTEGER_LITERAL_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::WILDCARD_LITERAL_EXPRESSION => {
                Some(Self::WildcardLiteralExpression(syntax))
            }
            SyntaxKind::REAL_LITERAL_EXPRESSION => Some(Self::RealLiteralExpression(syntax)),
            SyntaxKind::UNBASED_UNSIZED_LITERAL_EXPRESSION => {
                Some(Self::UnbasedUnsizedLiteralExpression(syntax))
            }
            SyntaxKind::NULL_LITERAL_EXPRESSION => Some(Self::NullLiteralExpression(syntax)),
            SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION => {
                Some(Self::DefaultPatternKeyExpression(syntax))
            }
            SyntaxKind::STRING_LITERAL_EXPRESSION => Some(Self::StringLiteralExpression(syntax)),
            SyntaxKind::TIME_LITERAL_EXPRESSION => Some(Self::TimeLiteralExpression(syntax)),
            SyntaxKind::INTEGER_LITERAL_EXPRESSION => Some(Self::IntegerLiteralExpression(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultRsCaseItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultRsCaseItem<'a> {
    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn item(&self) -> RsProdItem<'a> {
        self.syntax().child_node(2usize).and_then(RsProdItem::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for DefaultRsCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_RS_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnarySelectPropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UnarySelectPropertyExpr<'a> {
    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn selector(&self) -> Option<Selector<'a>> {
        self.syntax().child_node(2usize).and_then(Selector::cast)
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(4usize).and_then(PropertyExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for UnarySelectPropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNARY_SELECT_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionalStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConditionalStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn unique_or_priority(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn if_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn predicate(&self) -> ConditionalPredicate<'a> {
        self.syntax().child_node(5usize).and_then(ConditionalPredicate::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(7usize).and_then(Statement::cast).unwrap()
    }

    #[inline]
    pub fn else_clause(&self) -> Option<ElseClause<'a>> {
        self.syntax().child_node(8usize).and_then(ElseClause::cast)
    }
}
impl<'a> AstNode<'a> for ConditionalStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONDITIONAL_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueRangeExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ValueRangeExpression<'a> {
    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn right(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for ValueRangeExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::VALUE_RANGE_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuperNewDefaultedArgsExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SuperNewDefaultedArgsExpression<'a> {
    #[inline]
    pub fn scoped_new(&self) -> Name<'a> {
        self.syntax().child_node(0usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for SuperNewDefaultedArgsExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SUPER_NEW_DEFAULTED_ARGS_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimpleDirective<'a> {
    DelayModeDistributedDirective(SyntaxNode<'a>),
    DelayModeZeroDirective(SyntaxNode<'a>),
    ProtectDirective(SyntaxNode<'a>),
    UndefineAllDirective(SyntaxNode<'a>),
    CellDefineDirective(SyntaxNode<'a>),
    EndProtectDirective(SyntaxNode<'a>),
    DelayModeUnitDirective(SyntaxNode<'a>),
    EndCellDefineDirective(SyntaxNode<'a>),
    DelayModePathDirective(SyntaxNode<'a>),
    EndKeywordsDirective(SyntaxNode<'a>),
    NoUnconnectedDriveDirective(SyntaxNode<'a>),
    ResetAllDirective(SyntaxNode<'a>),
    ProtectedDirective(SyntaxNode<'a>),
    EndProtectedDirective(SyntaxNode<'a>),
}
impl<'a> SimpleDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn as_delay_mode_distributed_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DelayModeDistributedDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_delay_mode_zero_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DelayModeZeroDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_protect_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ProtectDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_undefine_all_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UndefineAllDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_cell_define_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::CellDefineDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_end_protect_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::EndProtectDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_delay_mode_unit_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DelayModeUnitDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_end_cell_define_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::EndCellDefineDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_delay_mode_path_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DelayModePathDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_end_keywords_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::EndKeywordsDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_no_unconnected_drive_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::NoUnconnectedDriveDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_reset_all_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ResetAllDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_protected_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ProtectedDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_end_protected_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::EndProtectedDirective(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for SimpleDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::DelayModeDistributedDirective(node) => *node,
            Self::DelayModeZeroDirective(node) => *node,
            Self::ProtectDirective(node) => *node,
            Self::UndefineAllDirective(node) => *node,
            Self::CellDefineDirective(node) => *node,
            Self::EndProtectDirective(node) => *node,
            Self::DelayModeUnitDirective(node) => *node,
            Self::EndCellDefineDirective(node) => *node,
            Self::DelayModePathDirective(node) => *node,
            Self::EndKeywordsDirective(node) => *node,
            Self::NoUnconnectedDriveDirective(node) => *node,
            Self::ResetAllDirective(node) => *node,
            Self::ProtectedDirective(node) => *node,
            Self::EndProtectedDirective(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE
            || kind == SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE
            || kind == SyntaxKind::PROTECT_DIRECTIVE
            || kind == SyntaxKind::UNDEFINE_ALL_DIRECTIVE
            || kind == SyntaxKind::CELL_DEFINE_DIRECTIVE
            || kind == SyntaxKind::END_PROTECT_DIRECTIVE
            || kind == SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE
            || kind == SyntaxKind::END_CELL_DEFINE_DIRECTIVE
            || kind == SyntaxKind::DELAY_MODE_PATH_DIRECTIVE
            || kind == SyntaxKind::END_KEYWORDS_DIRECTIVE
            || kind == SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE
            || kind == SyntaxKind::RESET_ALL_DIRECTIVE
            || kind == SyntaxKind::PROTECTED_DIRECTIVE
            || kind == SyntaxKind::END_PROTECTED_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE => {
                Some(Self::DelayModeDistributedDirective(syntax))
            }
            SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE => Some(Self::DelayModeZeroDirective(syntax)),
            SyntaxKind::PROTECT_DIRECTIVE => Some(Self::ProtectDirective(syntax)),
            SyntaxKind::UNDEFINE_ALL_DIRECTIVE => Some(Self::UndefineAllDirective(syntax)),
            SyntaxKind::CELL_DEFINE_DIRECTIVE => Some(Self::CellDefineDirective(syntax)),
            SyntaxKind::END_PROTECT_DIRECTIVE => Some(Self::EndProtectDirective(syntax)),
            SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE => Some(Self::DelayModeUnitDirective(syntax)),
            SyntaxKind::END_CELL_DEFINE_DIRECTIVE => Some(Self::EndCellDefineDirective(syntax)),
            SyntaxKind::DELAY_MODE_PATH_DIRECTIVE => Some(Self::DelayModePathDirective(syntax)),
            SyntaxKind::END_KEYWORDS_DIRECTIVE => Some(Self::EndKeywordsDirective(syntax)),
            SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE => {
                Some(Self::NoUnconnectedDriveDirective(syntax))
            }
            SyntaxKind::RESET_ALL_DIRECTIVE => Some(Self::ResetAllDirective(syntax)),
            SyntaxKind::PROTECTED_DIRECTIVE => Some(Self::ProtectedDirective(syntax)),
            SyntaxKind::END_PROTECTED_DIRECTIVE => Some(Self::EndProtectedDirective(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockingSequenceExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClockingSequenceExpr<'a> {
    #[inline]
    pub fn event(&self) -> TimingControl<'a> {
        self.syntax().child_node(0usize).and_then(TimingControl::cast).unwrap()
    }

    #[inline]
    pub fn expr(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(1usize).and_then(SequenceExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ClockingSequenceExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLOCKING_SEQUENCE_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpOutputPortDecl<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UdpOutputPortDecl<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn reg(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn initializer(&self) -> Option<EqualsValueClause<'a>> {
        self.syntax().child_node(4usize).and_then(EqualsValueClause::cast)
    }
}
impl<'a> AstNode<'a> for UdpOutputPortDecl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_OUTPUT_PORT_DECL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AcceptOnPropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AcceptOnPropertyExpr<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn condition(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(4usize).and_then(PropertyExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for AcceptOnPropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ACCEPT_ON_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathSuffix<'a> {
    SimplePathSuffix(SimplePathSuffix<'a>),
    EdgeSensitivePathSuffix(EdgeSensitivePathSuffix<'a>),
}
impl<'a> PathSuffix<'a> {
    #[inline]
    pub fn as_simple_path_suffix(self) -> Option<SimplePathSuffix<'a>> {
        match self {
            Self::SimplePathSuffix(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_edge_sensitive_path_suffix(self) -> Option<EdgeSensitivePathSuffix<'a>> {
        match self {
            Self::EdgeSensitivePathSuffix(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PathSuffix<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::SimplePathSuffix(node) => node.syntax(),
            Self::EdgeSensitivePathSuffix(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIMPLE_PATH_SUFFIX || kind == SyntaxKind::EDGE_SENSITIVE_PATH_SUFFIX
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::SIMPLE_PATH_SUFFIX => {
                Some(Self::SimplePathSuffix(SimplePathSuffix::cast(syntax).unwrap()))
            }
            SyntaxKind::EDGE_SENSITIVE_PATH_SUFFIX => {
                Some(Self::EdgeSensitivePathSuffix(EdgeSensitivePathSuffix::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyQueueExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EmptyQueueExpression<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for EmptyQueueExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_QUEUE_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NewClassExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NewClassExpression<'a> {
    #[inline]
    pub fn scoped_new(&self) -> Name<'a> {
        self.syntax().child_node(0usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn arg_list(&self) -> Option<ArgumentList<'a>> {
        self.syntax().child_node(1usize).and_then(ArgumentList::cast)
    }
}
impl<'a> AstNode<'a> for NewClassExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NEW_CLASS_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenthesizedEventExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenthesizedEventExpression<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> EventExpression<'a> {
        self.syntax().child_node(1usize).and_then(EventExpression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ParenthesizedEventExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_EVENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UniquenessConstraint<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UniquenessConstraint<'a> {
    #[inline]
    pub fn unique(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ranges(&self) -> RangeList<'a> {
        self.syntax().child_node(1usize).and_then(RangeList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for UniquenessConstraint<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNIQUENESS_CONSTRAINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OrderedArgument<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> OrderedArgument<'a> {
    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(0usize).and_then(PropertyExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for OrderedArgument<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ORDERED_ARGUMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassMethodPrototype<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClassMethodPrototype<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn qualifiers(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn prototype(&self) -> FunctionPrototype<'a> {
        self.syntax().child_node(2usize).and_then(FunctionPrototype::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ClassMethodPrototype<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLASS_METHOD_PROTOTYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InvocationExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> InvocationExpression<'a> {
    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn arguments(&self) -> Option<ArgumentList<'a>> {
        self.syntax().child_node(2usize).and_then(ArgumentList::cast)
    }
}
impl<'a> AstNode<'a> for InvocationExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INVOCATION_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnconnectedDriveDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UnconnectedDriveDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn strength(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for UnconnectedDriveDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNCONNECTED_DRIVE_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ForVariableDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ForVariableDeclaration<'a> {
    #[inline]
    pub fn var_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn type_(&self) -> Option<DataType<'a>> {
        self.syntax().child_node(1usize).and_then(DataType::cast)
    }

    #[inline]
    pub fn declarator(&self) -> Declarator<'a> {
        self.syntax().child_node(2usize).and_then(Declarator::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ForVariableDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FOR_VARIABLE_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageOption<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CoverageOption<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for CoverageOption<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVERAGE_OPTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockingItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClockingItem<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn direction(&self) -> ClockingDirection<'a> {
        self.syntax().child_node(1usize).and_then(ClockingDirection::cast).unwrap()
    }

    #[inline]
    pub fn decls(&self) -> SeparatedList<'a, AttributeSpec<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ClockingItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLOCKING_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrefixUnaryExpression<'a> {
    UnaryMinusExpression(SyntaxNode<'a>),
    UnaryBitwiseNandExpression(SyntaxNode<'a>),
    UnaryBitwiseXnorExpression(SyntaxNode<'a>),
    UnaryBitwiseXorExpression(SyntaxNode<'a>),
    UnaryPlusExpression(SyntaxNode<'a>),
    UnaryLogicalNotExpression(SyntaxNode<'a>),
    UnaryBitwiseOrExpression(SyntaxNode<'a>),
    UnaryBitwiseAndExpression(SyntaxNode<'a>),
    UnaryBitwiseNotExpression(SyntaxNode<'a>),
    UnaryPreincrementExpression(SyntaxNode<'a>),
    UnaryBitwiseNorExpression(SyntaxNode<'a>),
    UnaryPredecrementExpression(SyntaxNode<'a>),
}
impl<'a> PrefixUnaryExpression<'a> {
    #[inline]
    pub fn operator_token(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn operand(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn as_unary_minus_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryMinusExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_bitwise_nand_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryBitwiseNandExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_bitwise_xnor_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryBitwiseXnorExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_bitwise_xor_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryBitwiseXorExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_plus_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryPlusExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_logical_not_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryLogicalNotExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_bitwise_or_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryBitwiseOrExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_bitwise_and_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryBitwiseAndExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_bitwise_not_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryBitwiseNotExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_preincrement_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryPreincrementExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_bitwise_nor_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryBitwiseNorExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unary_predecrement_expression(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnaryPredecrementExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PrefixUnaryExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::UnaryMinusExpression(node) => *node,
            Self::UnaryBitwiseNandExpression(node) => *node,
            Self::UnaryBitwiseXnorExpression(node) => *node,
            Self::UnaryBitwiseXorExpression(node) => *node,
            Self::UnaryPlusExpression(node) => *node,
            Self::UnaryLogicalNotExpression(node) => *node,
            Self::UnaryBitwiseOrExpression(node) => *node,
            Self::UnaryBitwiseAndExpression(node) => *node,
            Self::UnaryBitwiseNotExpression(node) => *node,
            Self::UnaryPreincrementExpression(node) => *node,
            Self::UnaryBitwiseNorExpression(node) => *node,
            Self::UnaryPredecrementExpression(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNARY_MINUS_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_NAND_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_XNOR_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_XOR_EXPRESSION
            || kind == SyntaxKind::UNARY_PLUS_EXPRESSION
            || kind == SyntaxKind::UNARY_LOGICAL_NOT_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_OR_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_AND_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_NOT_EXPRESSION
            || kind == SyntaxKind::UNARY_PREINCREMENT_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_NOR_EXPRESSION
            || kind == SyntaxKind::UNARY_PREDECREMENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::UNARY_MINUS_EXPRESSION => Some(Self::UnaryMinusExpression(syntax)),
            SyntaxKind::UNARY_BITWISE_NAND_EXPRESSION => {
                Some(Self::UnaryBitwiseNandExpression(syntax))
            }
            SyntaxKind::UNARY_BITWISE_XNOR_EXPRESSION => {
                Some(Self::UnaryBitwiseXnorExpression(syntax))
            }
            SyntaxKind::UNARY_BITWISE_XOR_EXPRESSION => {
                Some(Self::UnaryBitwiseXorExpression(syntax))
            }
            SyntaxKind::UNARY_PLUS_EXPRESSION => Some(Self::UnaryPlusExpression(syntax)),
            SyntaxKind::UNARY_LOGICAL_NOT_EXPRESSION => {
                Some(Self::UnaryLogicalNotExpression(syntax))
            }
            SyntaxKind::UNARY_BITWISE_OR_EXPRESSION => Some(Self::UnaryBitwiseOrExpression(syntax)),
            SyntaxKind::UNARY_BITWISE_AND_EXPRESSION => {
                Some(Self::UnaryBitwiseAndExpression(syntax))
            }
            SyntaxKind::UNARY_BITWISE_NOT_EXPRESSION => {
                Some(Self::UnaryBitwiseNotExpression(syntax))
            }
            SyntaxKind::UNARY_PREINCREMENT_EXPRESSION => {
                Some(Self::UnaryPreincrementExpression(syntax))
            }
            SyntaxKind::UNARY_BITWISE_NOR_EXPRESSION => {
                Some(Self::UnaryBitwiseNorExpression(syntax))
            }
            SyntaxKind::UNARY_PREDECREMENT_EXPRESSION => {
                Some(Self::UnaryPredecrementExpression(syntax))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImplicitEventControl<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ImplicitEventControl<'a> {
    #[inline]
    pub fn at(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn star(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ImplicitEventControl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMPLICIT_EVENT_CONTROL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionBlock<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ActionBlock<'a> {
    #[inline]
    pub fn statement(&self) -> Option<Statement<'a>> {
        self.syntax().child_node(0usize).and_then(Statement::cast)
    }

    #[inline]
    pub fn else_clause(&self) -> Option<ElseClause<'a>> {
        self.syntax().child_node(1usize).and_then(ElseClause::cast)
    }
}
impl<'a> AstNode<'a> for ActionBlock<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ACTION_BLOCK
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImplicitNonAnsiPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ImplicitNonAnsiPort<'a> {
    #[inline]
    pub fn expr(&self) -> PortExpression<'a> {
        self.syntax().child_node(0usize).and_then(PortExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ImplicitNonAnsiPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMPLICIT_NON_ANSI_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModportSimplePortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ModportSimplePortList<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn direction(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn ports(&self) -> SeparatedList<'a, ModportPort<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ModportSimplePortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_SIMPLE_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockingSkew<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClockingSkew<'a> {
    #[inline]
    pub fn edge(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn delay(&self) -> Option<TimingControl<'a>> {
        self.syntax().child_node(1usize).and_then(TimingControl::cast)
    }
}
impl<'a> AstNode<'a> for ClockingSkew<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLOCKING_SKEW
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructUnionMember<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StructUnionMember<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn random_qualifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(2usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, Declarator<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for StructUnionMember<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STRUCT_UNION_MEMBER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ForeachLoopStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ForeachLoopStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn loop_list(&self) -> ForeachLoopList<'a> {
        self.syntax().child_node(3usize).and_then(ForeachLoopList::cast).unwrap()
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(4usize).and_then(Statement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ForeachLoopStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FOREACH_LOOP_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassMethodDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClassMethodDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn qualifiers(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn declaration(&self) -> FunctionDeclaration<'a> {
        self.syntax().child_node(2usize).and_then(FunctionDeclaration::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ClassMethodDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLASS_METHOD_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnsiPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AnsiPortList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ports(&self) -> SeparatedList<'a, Member<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for AnsiPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ANSI_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DistWeight<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DistWeight<'a> {
    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn extra_op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for DistWeight<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DIST_WEIGHT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Expression<'a> {
    BinaryExpression(BinaryExpression<'a>),
    DataType(DataType<'a>),
    TimingControlExpression(TimingControlExpression<'a>),
    PrimaryExpression(PrimaryExpression<'a>),
    BadExpression(BadExpression<'a>),
    PrefixUnaryExpression(PrefixUnaryExpression<'a>),
    Name(Name<'a>),
    CastExpression(CastExpression<'a>),
    MinTypMaxExpression(MinTypMaxExpression<'a>),
    CopyClassExpression(CopyClassExpression<'a>),
    ExpressionOrDist(ExpressionOrDist<'a>),
    ElementSelectExpression(ElementSelectExpression<'a>),
    SignedCastExpression(SignedCastExpression<'a>),
    PostfixUnaryExpression(PostfixUnaryExpression<'a>),
    InvocationExpression(InvocationExpression<'a>),
    NewArrayExpression(NewArrayExpression<'a>),
    NewClassExpression(NewClassExpression<'a>),
    ArrayOrRandomizeMethodExpression(ArrayOrRandomizeMethodExpression<'a>),
    InsideExpression(InsideExpression<'a>),
    MemberAccessExpression(MemberAccessExpression<'a>),
    ConditionalExpression(ConditionalExpression<'a>),
    ValueRangeExpression(ValueRangeExpression<'a>),
    TaggedUnionExpression(TaggedUnionExpression<'a>),
    SuperNewDefaultedArgsExpression(SuperNewDefaultedArgsExpression<'a>),
}
impl<'a> Expression<'a> {
    #[inline]
    pub fn as_binary_expression(self) -> Option<BinaryExpression<'a>> {
        match self {
            Self::BinaryExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_data_type(self) -> Option<DataType<'a>> {
        match self {
            Self::DataType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_timing_control_expression(self) -> Option<TimingControlExpression<'a>> {
        match self {
            Self::TimingControlExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_primary_expression(self) -> Option<PrimaryExpression<'a>> {
        match self {
            Self::PrimaryExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bad_expression(self) -> Option<BadExpression<'a>> {
        match self {
            Self::BadExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_prefix_unary_expression(self) -> Option<PrefixUnaryExpression<'a>> {
        match self {
            Self::PrefixUnaryExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_name(self) -> Option<Name<'a>> {
        match self {
            Self::Name(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_cast_expression(self) -> Option<CastExpression<'a>> {
        match self {
            Self::CastExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_min_typ_max_expression(self) -> Option<MinTypMaxExpression<'a>> {
        match self {
            Self::MinTypMaxExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_copy_class_expression(self) -> Option<CopyClassExpression<'a>> {
        match self {
            Self::CopyClassExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_expression_or_dist(self) -> Option<ExpressionOrDist<'a>> {
        match self {
            Self::ExpressionOrDist(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_element_select_expression(self) -> Option<ElementSelectExpression<'a>> {
        match self {
            Self::ElementSelectExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_signed_cast_expression(self) -> Option<SignedCastExpression<'a>> {
        match self {
            Self::SignedCastExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_postfix_unary_expression(self) -> Option<PostfixUnaryExpression<'a>> {
        match self {
            Self::PostfixUnaryExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_invocation_expression(self) -> Option<InvocationExpression<'a>> {
        match self {
            Self::InvocationExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_new_array_expression(self) -> Option<NewArrayExpression<'a>> {
        match self {
            Self::NewArrayExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_new_class_expression(self) -> Option<NewClassExpression<'a>> {
        match self {
            Self::NewClassExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_or_randomize_method_expression(
        self,
    ) -> Option<ArrayOrRandomizeMethodExpression<'a>> {
        match self {
            Self::ArrayOrRandomizeMethodExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_inside_expression(self) -> Option<InsideExpression<'a>> {
        match self {
            Self::InsideExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_member_access_expression(self) -> Option<MemberAccessExpression<'a>> {
        match self {
            Self::MemberAccessExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_conditional_expression(self) -> Option<ConditionalExpression<'a>> {
        match self {
            Self::ConditionalExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_value_range_expression(self) -> Option<ValueRangeExpression<'a>> {
        match self {
            Self::ValueRangeExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_tagged_union_expression(self) -> Option<TaggedUnionExpression<'a>> {
        match self {
            Self::TaggedUnionExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_super_new_defaulted_args_expression(
        self,
    ) -> Option<SuperNewDefaultedArgsExpression<'a>> {
        match self {
            Self::SuperNewDefaultedArgsExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Expression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::BinaryExpression(node) => node.syntax(),
            Self::DataType(node) => node.syntax(),
            Self::TimingControlExpression(node) => node.syntax(),
            Self::PrimaryExpression(node) => node.syntax(),
            Self::BadExpression(node) => node.syntax(),
            Self::PrefixUnaryExpression(node) => node.syntax(),
            Self::Name(node) => node.syntax(),
            Self::CastExpression(node) => node.syntax(),
            Self::MinTypMaxExpression(node) => node.syntax(),
            Self::CopyClassExpression(node) => node.syntax(),
            Self::ExpressionOrDist(node) => node.syntax(),
            Self::ElementSelectExpression(node) => node.syntax(),
            Self::SignedCastExpression(node) => node.syntax(),
            Self::PostfixUnaryExpression(node) => node.syntax(),
            Self::InvocationExpression(node) => node.syntax(),
            Self::NewArrayExpression(node) => node.syntax(),
            Self::NewClassExpression(node) => node.syntax(),
            Self::ArrayOrRandomizeMethodExpression(node) => node.syntax(),
            Self::InsideExpression(node) => node.syntax(),
            Self::MemberAccessExpression(node) => node.syntax(),
            Self::ConditionalExpression(node) => node.syntax(),
            Self::ValueRangeExpression(node) => node.syntax(),
            Self::TaggedUnionExpression(node) => node.syntax(),
            Self::SuperNewDefaultedArgsExpression(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MOD_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::ARITHMETIC_SHIFT_RIGHT_EXPRESSION
            || kind == SyntaxKind::REAL_TYPE
            || kind == SyntaxKind::EVENT_TYPE
            || kind == SyntaxKind::BINARY_AND_EXPRESSION
            || kind == SyntaxKind::BIT_TYPE
            || kind == SyntaxKind::DIVIDE_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::LOGICAL_AND_EXPRESSION
            || kind == SyntaxKind::REG_TYPE
            || kind == SyntaxKind::INEQUALITY_EXPRESSION
            || kind == SyntaxKind::SEQUENCE_TYPE
            || kind == SyntaxKind::GREATER_THAN_EXPRESSION
            || kind == SyntaxKind::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::TIMING_CONTROL_EXPRESSION
            || kind == SyntaxKind::LONG_INT_TYPE
            || kind == SyntaxKind::PARENTHESIZED_EXPRESSION
            || kind == SyntaxKind::BAD_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_XNOR_EXPRESSION
            || kind == SyntaxKind::ENUM_TYPE
            || kind == SyntaxKind::CONSTRUCTOR_NAME
            || kind == SyntaxKind::TIME_LITERAL_EXPRESSION
            || kind == SyntaxKind::AND_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::LOGICAL_EQUIVALENCE_EXPRESSION
            || kind == SyntaxKind::UNARY_PREDECREMENT_EXPRESSION
            || kind == SyntaxKind::BINARY_OR_EXPRESSION
            || kind == SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION
            || kind == SyntaxKind::LOGIC_TYPE
            || kind == SyntaxKind::LOGICAL_OR_EXPRESSION
            || kind == SyntaxKind::IDENTIFIER_SELECT_NAME
            || kind == SyntaxKind::NULL_LITERAL_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_OR_EXPRESSION
            || kind == SyntaxKind::UNBASED_UNSIZED_LITERAL_EXPRESSION
            || kind == SyntaxKind::STREAMING_CONCATENATION_EXPRESSION
            || kind == SyntaxKind::BYTE_TYPE
            || kind == SyntaxKind::XOR_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::SUBTRACT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::UNION_TYPE
            || kind == SyntaxKind::UNARY_BITWISE_NAND_EXPRESSION
            || kind == SyntaxKind::POWER_EXPRESSION
            || kind == SyntaxKind::UNTYPED
            || kind == SyntaxKind::MULTIPLE_CONCATENATION_EXPRESSION
            || kind == SyntaxKind::MULTIPLY_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::SUPER_HANDLE
            || kind == SyntaxKind::CAST_EXPRESSION
            || kind == SyntaxKind::MIN_TYP_MAX_EXPRESSION
            || kind == SyntaxKind::SUBTRACT_EXPRESSION
            || kind == SyntaxKind::COPY_CLASS_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_AND_EXPRESSION
            || kind == SyntaxKind::SHORT_REAL_TYPE
            || kind == SyntaxKind::EXPRESSION_OR_DIST
            || kind == SyntaxKind::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::UNIT_SCOPE
            || kind == SyntaxKind::REAL_LITERAL_EXPRESSION
            || kind == SyntaxKind::EQUALITY_EXPRESSION
            || kind == SyntaxKind::OR_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::ELEMENT_SELECT_EXPRESSION
            || kind == SyntaxKind::SIGNED_CAST_EXPRESSION
            || kind == SyntaxKind::TYPE_REFERENCE
            || kind == SyntaxKind::POSTINCREMENT_EXPRESSION
            || kind == SyntaxKind::TIME_TYPE
            || kind == SyntaxKind::IMPLICIT_TYPE
            || kind == SyntaxKind::INVOCATION_EXPRESSION
            || kind == SyntaxKind::NEW_ARRAY_EXPRESSION
            || kind == SyntaxKind::WILDCARD_LITERAL_EXPRESSION
            || kind == SyntaxKind::NEW_CLASS_EXPRESSION
            || kind == SyntaxKind::ARRAY_OR_RANDOMIZE_METHOD_EXPRESSION
            || kind == SyntaxKind::CASE_INEQUALITY_EXPRESSION
            || kind == SyntaxKind::IDENTIFIER_NAME
            || kind == SyntaxKind::WILDCARD_INEQUALITY_EXPRESSION
            || kind == SyntaxKind::LOGICAL_IMPLICATION_EXPRESSION
            || kind == SyntaxKind::ADD_EXPRESSION
            || kind == SyntaxKind::MULTIPLY_EXPRESSION
            || kind == SyntaxKind::CLASS_NAME
            || kind == SyntaxKind::ARRAY_AND_METHOD
            || kind == SyntaxKind::ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::LOGICAL_SHIFT_LEFT_EXPRESSION
            || kind == SyntaxKind::ARRAY_UNIQUE_METHOD
            || kind == SyntaxKind::EMPTY_QUEUE_EXPRESSION
            || kind == SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION
            || kind == SyntaxKind::SHORT_INT_TYPE
            || kind == SyntaxKind::LESS_THAN_EQUAL_EXPRESSION
            || kind == SyntaxKind::INTEGER_TYPE
            || kind == SyntaxKind::CASE_EQUALITY_EXPRESSION
            || kind == SyntaxKind::BINARY_XNOR_EXPRESSION
            || kind == SyntaxKind::LOCAL_SCOPE
            || kind == SyntaxKind::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::ARITHMETIC_SHIFT_LEFT_EXPRESSION
            || kind == SyntaxKind::ADD_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::DIVIDE_EXPRESSION
            || kind == SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::LESS_THAN_EXPRESSION
            || kind == SyntaxKind::PROPERTY_TYPE
            || kind == SyntaxKind::BINARY_XOR_EXPRESSION
            || kind == SyntaxKind::STRING_LITERAL_EXPRESSION
            || kind == SyntaxKind::NAMED_TYPE
            || kind == SyntaxKind::UNARY_PREINCREMENT_EXPRESSION
            || kind == SyntaxKind::ROOT_SCOPE
            || kind == SyntaxKind::UNARY_MINUS_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_XOR_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_NOT_EXPRESSION
            || kind == SyntaxKind::INSIDE_EXPRESSION
            || kind == SyntaxKind::STRUCT_TYPE
            || kind == SyntaxKind::STRING_TYPE
            || kind == SyntaxKind::VIRTUAL_INTERFACE_TYPE
            || kind == SyntaxKind::THIS_HANDLE
            || kind == SyntaxKind::SCOPED_NAME
            || kind == SyntaxKind::MEMBER_ACCESS_EXPRESSION
            || kind == SyntaxKind::INTEGER_LITERAL_EXPRESSION
            || kind == SyntaxKind::EMPTY_IDENTIFIER_NAME
            || kind == SyntaxKind::INT_TYPE
            || kind == SyntaxKind::CONCATENATION_EXPRESSION
            || kind == SyntaxKind::C_HANDLE_TYPE
            || kind == SyntaxKind::SYSTEM_NAME
            || kind == SyntaxKind::ARRAY_OR_METHOD
            || kind == SyntaxKind::CONDITIONAL_EXPRESSION
            || kind == SyntaxKind::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
            || kind == SyntaxKind::VALUE_RANGE_EXPRESSION
            || kind == SyntaxKind::WILDCARD_EQUALITY_EXPRESSION
            || kind == SyntaxKind::INTEGER_VECTOR_EXPRESSION
            || kind == SyntaxKind::LOGICAL_SHIFT_RIGHT_EXPRESSION
            || kind == SyntaxKind::GREATER_THAN_EQUAL_EXPRESSION
            || kind == SyntaxKind::TAGGED_UNION_EXPRESSION
            || kind == SyntaxKind::UNARY_LOGICAL_NOT_EXPRESSION
            || kind == SyntaxKind::MOD_EXPRESSION
            || kind == SyntaxKind::UNARY_BITWISE_NOR_EXPRESSION
            || kind == SyntaxKind::SUPER_NEW_DEFAULTED_ARGS_EXPRESSION
            || kind == SyntaxKind::VOID_TYPE
            || kind == SyntaxKind::UNARY_PLUS_EXPRESSION
            || kind == SyntaxKind::REAL_TIME_TYPE
            || kind == SyntaxKind::POSTDECREMENT_EXPRESSION
            || kind == SyntaxKind::ARRAY_XOR_METHOD
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::MOD_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ARITHMETIC_SHIFT_RIGHT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::REAL_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::EVENT_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::BINARY_AND_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::BIT_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::DIVIDE_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LOGICAL_AND_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::REG_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::INEQUALITY_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SEQUENCE_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::GREATER_THAN_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::TIMING_CONTROL_EXPRESSION => {
                Some(Self::TimingControlExpression(TimingControlExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LONG_INT_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::PARENTHESIZED_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::BAD_EXPRESSION => {
                Some(Self::BadExpression(BadExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_BITWISE_XNOR_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ENUM_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::CONSTRUCTOR_NAME => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::TIME_LITERAL_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::AND_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LOGICAL_EQUIVALENCE_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_PREDECREMENT_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::BINARY_OR_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LOGIC_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::LOGICAL_OR_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::IDENTIFIER_SELECT_NAME => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::NULL_LITERAL_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_BITWISE_OR_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNBASED_UNSIZED_LITERAL_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::STREAMING_CONCATENATION_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::BYTE_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::XOR_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SUBTRACT_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNION_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::UNARY_BITWISE_NAND_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::POWER_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNTYPED => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::MULTIPLE_CONCATENATION_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::MULTIPLY_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SUPER_HANDLE => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::CAST_EXPRESSION => {
                Some(Self::CastExpression(CastExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::MIN_TYP_MAX_EXPRESSION => {
                Some(Self::MinTypMaxExpression(MinTypMaxExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SUBTRACT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::COPY_CLASS_EXPRESSION => {
                Some(Self::CopyClassExpression(CopyClassExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_BITWISE_AND_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SHORT_REAL_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::EXPRESSION_OR_DIST => {
                Some(Self::ExpressionOrDist(ExpressionOrDist::cast(syntax).unwrap()))
            }
            SyntaxKind::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNIT_SCOPE => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::REAL_LITERAL_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::EQUALITY_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::OR_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ELEMENT_SELECT_EXPRESSION => {
                Some(Self::ElementSelectExpression(ElementSelectExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SIGNED_CAST_EXPRESSION => {
                Some(Self::SignedCastExpression(SignedCastExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::TYPE_REFERENCE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::POSTINCREMENT_EXPRESSION => {
                Some(Self::PostfixUnaryExpression(PostfixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::TIME_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::IMPLICIT_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::INVOCATION_EXPRESSION => {
                Some(Self::InvocationExpression(InvocationExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::NEW_ARRAY_EXPRESSION => {
                Some(Self::NewArrayExpression(NewArrayExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::WILDCARD_LITERAL_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::NEW_CLASS_EXPRESSION => {
                Some(Self::NewClassExpression(NewClassExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ARRAY_OR_RANDOMIZE_METHOD_EXPRESSION => {
                Some(Self::ArrayOrRandomizeMethodExpression(
                    ArrayOrRandomizeMethodExpression::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::CASE_INEQUALITY_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::IDENTIFIER_NAME => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::WILDCARD_INEQUALITY_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LOGICAL_IMPLICATION_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ADD_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::MULTIPLY_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::CLASS_NAME => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::ARRAY_AND_METHOD => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LOGICAL_SHIFT_LEFT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ARRAY_UNIQUE_METHOD => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::EMPTY_QUEUE_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SHORT_INT_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::LESS_THAN_EQUAL_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::INTEGER_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::CASE_EQUALITY_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::BINARY_XNOR_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LOCAL_SCOPE => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ARITHMETIC_SHIFT_LEFT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ADD_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::DIVIDE_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LESS_THAN_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::PROPERTY_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::BINARY_XOR_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::STRING_LITERAL_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::NAMED_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::UNARY_PREINCREMENT_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ROOT_SCOPE => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::UNARY_MINUS_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_BITWISE_XOR_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_BITWISE_NOT_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::INSIDE_EXPRESSION => {
                Some(Self::InsideExpression(InsideExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::STRUCT_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::STRING_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::VIRTUAL_INTERFACE_TYPE => {
                Some(Self::DataType(DataType::cast(syntax).unwrap()))
            }
            SyntaxKind::THIS_HANDLE => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::SCOPED_NAME => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::MEMBER_ACCESS_EXPRESSION => {
                Some(Self::MemberAccessExpression(MemberAccessExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::INTEGER_LITERAL_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::EMPTY_IDENTIFIER_NAME => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::INT_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::CONCATENATION_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::C_HANDLE_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::SYSTEM_NAME => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::ARRAY_OR_METHOD => Some(Self::Name(Name::cast(syntax).unwrap())),
            SyntaxKind::CONDITIONAL_EXPRESSION => {
                Some(Self::ConditionalExpression(ConditionalExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::VALUE_RANGE_EXPRESSION => {
                Some(Self::ValueRangeExpression(ValueRangeExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::WILDCARD_EQUALITY_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::INTEGER_VECTOR_EXPRESSION => {
                Some(Self::PrimaryExpression(PrimaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::LOGICAL_SHIFT_RIGHT_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::GREATER_THAN_EQUAL_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::TAGGED_UNION_EXPRESSION => {
                Some(Self::TaggedUnionExpression(TaggedUnionExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_LOGICAL_NOT_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::MOD_EXPRESSION => {
                Some(Self::BinaryExpression(BinaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNARY_BITWISE_NOR_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SUPER_NEW_DEFAULTED_ARGS_EXPRESSION => {
                Some(Self::SuperNewDefaultedArgsExpression(
                    SuperNewDefaultedArgsExpression::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::VOID_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::UNARY_PLUS_EXPRESSION => {
                Some(Self::PrefixUnaryExpression(PrefixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::REAL_TIME_TYPE => Some(Self::DataType(DataType::cast(syntax).unwrap())),
            SyntaxKind::POSTDECREMENT_EXPRESSION => {
                Some(Self::PostfixUnaryExpression(PostfixUnaryExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ARRAY_XOR_METHOD => Some(Self::Name(Name::cast(syntax).unwrap())),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElsePropertyClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ElsePropertyClause<'a> {
    #[inline]
    pub fn else_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(1usize).and_then(PropertyExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ElsePropertyClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ELSE_PROPERTY_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ForLoopStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ForLoopStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn for_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn initializers(&self) -> SeparatedList<'a, HybridNode<'a>> {
        self.syntax().child_node(4usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn stop_expr(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(6usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn semi_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }

    #[inline]
    pub fn steps(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(8usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(9usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(10usize).and_then(Statement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ForLoopStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FOR_LOOP_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RangeSelect<'a> {
    DescendingRangeSelect(SyntaxNode<'a>),
    AscendingRangeSelect(SyntaxNode<'a>),
    SimpleRangeSelect(SyntaxNode<'a>),
}
impl<'a> RangeSelect<'a> {
    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn range(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn as_descending_range_select(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::DescendingRangeSelect(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ascending_range_select(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AscendingRangeSelect(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_simple_range_select(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::SimpleRangeSelect(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for RangeSelect<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::DescendingRangeSelect(node) => *node,
            Self::AscendingRangeSelect(node) => *node,
            Self::SimpleRangeSelect(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DESCENDING_RANGE_SELECT
            || kind == SyntaxKind::ASCENDING_RANGE_SELECT
            || kind == SyntaxKind::SIMPLE_RANGE_SELECT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::DESCENDING_RANGE_SELECT => Some(Self::DescendingRangeSelect(syntax)),
            SyntaxKind::ASCENDING_RANGE_SELECT => Some(Self::AscendingRangeSelect(syntax)),
            SyntaxKind::SIMPLE_RANGE_SELECT => Some(Self::SimpleRangeSelect(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HierarchicalInstance<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> HierarchicalInstance<'a> {
    #[inline]
    pub fn decl(&self) -> Option<InstanceName<'a>> {
        self.syntax().child_node(0usize).and_then(InstanceName::cast)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn connections(&self) -> SeparatedList<'a, PortConnection<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for HierarchicalInstance<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::HIERARCHICAL_INSTANCE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExtendsClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExtendsClause<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn base_name(&self) -> Name<'a> {
        self.syntax().child_node(1usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn arguments(&self) -> Option<ArgumentList<'a>> {
        self.syntax().child_node(2usize).and_then(ArgumentList::cast)
    }

    #[inline]
    pub fn defaulted_arg(&self) -> Option<DefaultExtendsClauseArg<'a>> {
        self.syntax().child_node(3usize).and_then(DefaultExtendsClauseArg::cast)
    }
}
impl<'a> AstNode<'a> for ExtendsClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXTENDS_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WithClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WithClause<'a> {
    #[inline]
    pub fn with(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for WithClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WITH_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinaryEventExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BinaryEventExpression<'a> {
    #[inline]
    pub fn left(&self) -> EventExpression<'a> {
        self.syntax().child_node(0usize).and_then(EventExpression::cast).unwrap()
    }

    #[inline]
    pub fn operator_token(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> EventExpression<'a> {
        self.syntax().child_node(2usize).and_then(EventExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for BinaryEventExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BINARY_EVENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParameterDeclarationStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParameterDeclarationStatement<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn parameter(&self) -> ParameterDeclarationBase<'a> {
        self.syntax().child_node(1usize).and_then(ParameterDeclarationBase::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ParameterDeclarationStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARAMETER_DECLARATION_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Production<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> Production<'a> {
    #[inline]
    pub fn data_type(&self) -> Option<DataType<'a>> {
        self.syntax().child_node(0usize).and_then(DataType::cast)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn port_list(&self) -> Option<FunctionPortList<'a>> {
        self.syntax().child_node(2usize).and_then(FunctionPortList::cast)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn rules(&self) -> SeparatedList<'a, RsRule<'a>> {
        self.syntax().child_node(4usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for Production<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PRODUCTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConstraintPrototype<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConstraintPrototype<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn qualifiers(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn specifiers(&self) -> SyntaxList<'a, ClassSpecifier<'a>> {
        self.syntax().child_node(3usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(4usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for ConstraintPrototype<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONSTRAINT_PROTOTYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NetPortHeader<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NetPortHeader<'a> {
    #[inline]
    pub fn direction(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn net_type(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn data_type(&self) -> DataType<'a> {
        self.syntax().child_node(2usize).and_then(DataType::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for NetPortHeader<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NET_PORT_HEADER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CheckerDataDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CheckerDataDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn rand(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn data(&self) -> DataDeclaration<'a> {
        self.syntax().child_node(2usize).and_then(DataDeclaration::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for CheckerDataDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CHECKER_DATA_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnconditionalBranchDirective<'a> {
    EndIfDirective(SyntaxNode<'a>),
    ElseDirective(SyntaxNode<'a>),
}
impl<'a> UnconditionalBranchDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn disabled_tokens(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn as_end_if_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::EndIfDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_else_directive(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ElseDirective(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for UnconditionalBranchDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::EndIfDirective(node) => *node,
            Self::ElseDirective(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::END_IF_DIRECTIVE || kind == SyntaxKind::ELSE_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::END_IF_DIRECTIVE => Some(Self::EndIfDirective(syntax)),
            SyntaxKind::ELSE_DIRECTIVE => Some(Self::ElseDirective(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultNetTypeDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultNetTypeDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn net_type(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for DefaultNetTypeDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssignmentPatternExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AssignmentPatternExpression<'a> {
    #[inline]
    pub fn type_(&self) -> Option<DataType<'a>> {
        self.syntax().child_node(0usize).and_then(DataType::cast)
    }

    #[inline]
    pub fn pattern(&self) -> AssignmentPattern<'a> {
        self.syntax().child_node(1usize).and_then(AssignmentPattern::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for AssignmentPatternExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VariablePortHeader<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> VariablePortHeader<'a> {
    #[inline]
    pub fn const_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn direction(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn var_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn data_type(&self) -> DataType<'a> {
        self.syntax().child_node(3usize).and_then(DataType::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for VariablePortHeader<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::VARIABLE_PORT_HEADER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnsiUdpPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AnsiUdpPortList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ports(&self) -> SeparatedList<'a, UdpPortDecl<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for AnsiUdpPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ANSI_UDP_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageBinsArraySize<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CoverageBinsArraySize<'a> {
    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(1usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for CoverageBinsArraySize<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVERAGE_BINS_ARRAY_SIZE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NewArrayExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NewArrayExpression<'a> {
    #[inline]
    pub fn new_keyword(&self) -> Name<'a> {
        self.syntax().child_node(0usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn size_expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn initializer(&self) -> Option<ParenthesizedExpression<'a>> {
        self.syntax().child_node(4usize).and_then(ParenthesizedExpression::cast)
    }
}
impl<'a> AstNode<'a> for NewArrayExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NEW_ARRAY_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultCaseItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultCaseItem<'a> {
    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn clause(&self) -> HybridNode<'a> {
        self.syntax().child_node(2usize).and_then(HybridNode::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for DefaultCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UserDefinedNetDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UserDefinedNetDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn net_type(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn delay(&self) -> TimingControl<'a> {
        self.syntax().child_node(2usize).and_then(TimingControl::cast).unwrap()
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, Declarator<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for UserDefinedNetDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::USER_DEFINED_NET_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimingCheckEventArg<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TimingCheckEventArg<'a> {
    #[inline]
    pub fn edge(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn control_specifier(&self) -> Option<EdgeControlSpecifier<'a>> {
        self.syntax().child_node(1usize).and_then(EdgeControlSpecifier::cast)
    }

    #[inline]
    pub fn terminal(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn condition(&self) -> Option<TimingCheckEventCondition<'a>> {
        self.syntax().child_node(3usize).and_then(TimingCheckEventCondition::cast)
    }
}
impl<'a> AstNode<'a> for TimingCheckEventArg<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TIMING_CHECK_EVENT_ARG
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImplicationConstraint<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ImplicationConstraint<'a> {
    #[inline]
    pub fn left(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn arrow(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn constraints(&self) -> ConstraintItem<'a> {
        self.syntax().child_node(2usize).and_then(ConstraintItem::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ImplicationConstraint<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMPLICATION_CONSTRAINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FunctionDeclaration<'a> {
    FunctionDeclaration(SyntaxNode<'a>),
    TaskDeclaration(SyntaxNode<'a>),
}
impl<'a> FunctionDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn prototype(&self) -> FunctionPrototype<'a> {
        self.syntax().child_node(1usize).and_then(FunctionPrototype::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, HybridNode<'a>> {
        self.syntax().child_node(3usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(5usize).and_then(NamedBlockClause::cast)
    }

    #[inline]
    pub fn as_function_declaration(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::FunctionDeclaration(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_task_declaration(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::TaskDeclaration(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for FunctionDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::FunctionDeclaration(node) => *node,
            Self::TaskDeclaration(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FUNCTION_DECLARATION || kind == SyntaxKind::TASK_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::FUNCTION_DECLARATION => Some(Self::FunctionDeclaration(syntax)),
            SyntaxKind::TASK_DECLARATION => Some(Self::TaskDeclaration(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Statement<'a> {
    ConcurrentAssertionStatement(ConcurrentAssertionStatement<'a>),
    JumpStatement(JumpStatement<'a>),
    ForeachLoopStatement(ForeachLoopStatement<'a>),
    ReturnStatement(ReturnStatement<'a>),
    ProceduralDeassignStatement(ProceduralDeassignStatement<'a>),
    WaitStatement(WaitStatement<'a>),
    ExpressionStatement(ExpressionStatement<'a>),
    ProceduralAssignStatement(ProceduralAssignStatement<'a>),
    WaitOrderStatement(WaitOrderStatement<'a>),
    LoopStatement(LoopStatement<'a>),
    EventTriggerStatement(EventTriggerStatement<'a>),
    CheckerInstanceStatement(CheckerInstanceStatement<'a>),
    EmptyStatement(EmptyStatement<'a>),
    VoidCastedCallStatement(VoidCastedCallStatement<'a>),
    RandSequenceStatement(RandSequenceStatement<'a>),
    WaitForkStatement(WaitForkStatement<'a>),
    DisableForkStatement(DisableForkStatement<'a>),
    TimingControlStatement(TimingControlStatement<'a>),
    RandCaseStatement(RandCaseStatement<'a>),
    BlockStatement(BlockStatement<'a>),
    ForeverStatement(ForeverStatement<'a>),
    ConditionalStatement(ConditionalStatement<'a>),
    ImmediateAssertionStatement(ImmediateAssertionStatement<'a>),
    DisableStatement(DisableStatement<'a>),
    DoWhileStatement(DoWhileStatement<'a>),
    CaseStatement(CaseStatement<'a>),
    ForLoopStatement(ForLoopStatement<'a>),
}
impl<'a> Statement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn as_concurrent_assertion_statement(self) -> Option<ConcurrentAssertionStatement<'a>> {
        match self {
            Self::ConcurrentAssertionStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_jump_statement(self) -> Option<JumpStatement<'a>> {
        match self {
            Self::JumpStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_foreach_loop_statement(self) -> Option<ForeachLoopStatement<'a>> {
        match self {
            Self::ForeachLoopStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_return_statement(self) -> Option<ReturnStatement<'a>> {
        match self {
            Self::ReturnStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_procedural_deassign_statement(self) -> Option<ProceduralDeassignStatement<'a>> {
        match self {
            Self::ProceduralDeassignStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_wait_statement(self) -> Option<WaitStatement<'a>> {
        match self {
            Self::WaitStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_expression_statement(self) -> Option<ExpressionStatement<'a>> {
        match self {
            Self::ExpressionStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_procedural_assign_statement(self) -> Option<ProceduralAssignStatement<'a>> {
        match self {
            Self::ProceduralAssignStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_wait_order_statement(self) -> Option<WaitOrderStatement<'a>> {
        match self {
            Self::WaitOrderStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_loop_statement(self) -> Option<LoopStatement<'a>> {
        match self {
            Self::LoopStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_event_trigger_statement(self) -> Option<EventTriggerStatement<'a>> {
        match self {
            Self::EventTriggerStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_checker_instance_statement(self) -> Option<CheckerInstanceStatement<'a>> {
        match self {
            Self::CheckerInstanceStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_empty_statement(self) -> Option<EmptyStatement<'a>> {
        match self {
            Self::EmptyStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_void_casted_call_statement(self) -> Option<VoidCastedCallStatement<'a>> {
        match self {
            Self::VoidCastedCallStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_rand_sequence_statement(self) -> Option<RandSequenceStatement<'a>> {
        match self {
            Self::RandSequenceStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_wait_fork_statement(self) -> Option<WaitForkStatement<'a>> {
        match self {
            Self::WaitForkStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_disable_fork_statement(self) -> Option<DisableForkStatement<'a>> {
        match self {
            Self::DisableForkStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_timing_control_statement(self) -> Option<TimingControlStatement<'a>> {
        match self {
            Self::TimingControlStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_rand_case_statement(self) -> Option<RandCaseStatement<'a>> {
        match self {
            Self::RandCaseStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_block_statement(self) -> Option<BlockStatement<'a>> {
        match self {
            Self::BlockStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_forever_statement(self) -> Option<ForeverStatement<'a>> {
        match self {
            Self::ForeverStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_conditional_statement(self) -> Option<ConditionalStatement<'a>> {
        match self {
            Self::ConditionalStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_immediate_assertion_statement(self) -> Option<ImmediateAssertionStatement<'a>> {
        match self {
            Self::ImmediateAssertionStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_disable_statement(self) -> Option<DisableStatement<'a>> {
        match self {
            Self::DisableStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_do_while_statement(self) -> Option<DoWhileStatement<'a>> {
        match self {
            Self::DoWhileStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_case_statement(self) -> Option<CaseStatement<'a>> {
        match self {
            Self::CaseStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_for_loop_statement(self) -> Option<ForLoopStatement<'a>> {
        match self {
            Self::ForLoopStatement(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Statement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ConcurrentAssertionStatement(node) => node.syntax(),
            Self::JumpStatement(node) => node.syntax(),
            Self::ForeachLoopStatement(node) => node.syntax(),
            Self::ReturnStatement(node) => node.syntax(),
            Self::ProceduralDeassignStatement(node) => node.syntax(),
            Self::WaitStatement(node) => node.syntax(),
            Self::ExpressionStatement(node) => node.syntax(),
            Self::ProceduralAssignStatement(node) => node.syntax(),
            Self::WaitOrderStatement(node) => node.syntax(),
            Self::LoopStatement(node) => node.syntax(),
            Self::EventTriggerStatement(node) => node.syntax(),
            Self::CheckerInstanceStatement(node) => node.syntax(),
            Self::EmptyStatement(node) => node.syntax(),
            Self::VoidCastedCallStatement(node) => node.syntax(),
            Self::RandSequenceStatement(node) => node.syntax(),
            Self::WaitForkStatement(node) => node.syntax(),
            Self::DisableForkStatement(node) => node.syntax(),
            Self::TimingControlStatement(node) => node.syntax(),
            Self::RandCaseStatement(node) => node.syntax(),
            Self::BlockStatement(node) => node.syntax(),
            Self::ForeverStatement(node) => node.syntax(),
            Self::ConditionalStatement(node) => node.syntax(),
            Self::ImmediateAssertionStatement(node) => node.syntax(),
            Self::DisableStatement(node) => node.syntax(),
            Self::DoWhileStatement(node) => node.syntax(),
            Self::CaseStatement(node) => node.syntax(),
            Self::ForLoopStatement(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVER_SEQUENCE_STATEMENT
            || kind == SyntaxKind::JUMP_STATEMENT
            || kind == SyntaxKind::FOREACH_LOOP_STATEMENT
            || kind == SyntaxKind::RETURN_STATEMENT
            || kind == SyntaxKind::PROCEDURAL_RELEASE_STATEMENT
            || kind == SyntaxKind::WAIT_STATEMENT
            || kind == SyntaxKind::EXPRESSION_STATEMENT
            || kind == SyntaxKind::PROCEDURAL_FORCE_STATEMENT
            || kind == SyntaxKind::WAIT_ORDER_STATEMENT
            || kind == SyntaxKind::RESTRICT_PROPERTY_STATEMENT
            || kind == SyntaxKind::PROCEDURAL_DEASSIGN_STATEMENT
            || kind == SyntaxKind::LOOP_STATEMENT
            || kind == SyntaxKind::NONBLOCKING_EVENT_TRIGGER_STATEMENT
            || kind == SyntaxKind::CHECKER_INSTANCE_STATEMENT
            || kind == SyntaxKind::EMPTY_STATEMENT
            || kind == SyntaxKind::VOID_CASTED_CALL_STATEMENT
            || kind == SyntaxKind::RAND_SEQUENCE_STATEMENT
            || kind == SyntaxKind::BLOCKING_EVENT_TRIGGER_STATEMENT
            || kind == SyntaxKind::WAIT_FORK_STATEMENT
            || kind == SyntaxKind::DISABLE_FORK_STATEMENT
            || kind == SyntaxKind::TIMING_CONTROL_STATEMENT
            || kind == SyntaxKind::RAND_CASE_STATEMENT
            || kind == SyntaxKind::PARALLEL_BLOCK_STATEMENT
            || kind == SyntaxKind::FOREVER_STATEMENT
            || kind == SyntaxKind::CONDITIONAL_STATEMENT
            || kind == SyntaxKind::IMMEDIATE_ASSUME_STATEMENT
            || kind == SyntaxKind::ASSUME_PROPERTY_STATEMENT
            || kind == SyntaxKind::IMMEDIATE_COVER_STATEMENT
            || kind == SyntaxKind::COVER_PROPERTY_STATEMENT
            || kind == SyntaxKind::ASSERT_PROPERTY_STATEMENT
            || kind == SyntaxKind::EXPECT_PROPERTY_STATEMENT
            || kind == SyntaxKind::DISABLE_STATEMENT
            || kind == SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT
            || kind == SyntaxKind::IMMEDIATE_ASSERT_STATEMENT
            || kind == SyntaxKind::PROCEDURAL_ASSIGN_STATEMENT
            || kind == SyntaxKind::DO_WHILE_STATEMENT
            || kind == SyntaxKind::CASE_STATEMENT
            || kind == SyntaxKind::FOR_LOOP_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::COVER_SEQUENCE_STATEMENT => Some(Self::ConcurrentAssertionStatement(
                ConcurrentAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::JUMP_STATEMENT => {
                Some(Self::JumpStatement(JumpStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::FOREACH_LOOP_STATEMENT => {
                Some(Self::ForeachLoopStatement(ForeachLoopStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::RETURN_STATEMENT => {
                Some(Self::ReturnStatement(ReturnStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::PROCEDURAL_RELEASE_STATEMENT => Some(Self::ProceduralDeassignStatement(
                ProceduralDeassignStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::WAIT_STATEMENT => {
                Some(Self::WaitStatement(WaitStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::EXPRESSION_STATEMENT => {
                Some(Self::ExpressionStatement(ExpressionStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::PROCEDURAL_FORCE_STATEMENT => Some(Self::ProceduralAssignStatement(
                ProceduralAssignStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::WAIT_ORDER_STATEMENT => {
                Some(Self::WaitOrderStatement(WaitOrderStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::RESTRICT_PROPERTY_STATEMENT => Some(Self::ConcurrentAssertionStatement(
                ConcurrentAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::PROCEDURAL_DEASSIGN_STATEMENT => Some(Self::ProceduralDeassignStatement(
                ProceduralDeassignStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::LOOP_STATEMENT => {
                Some(Self::LoopStatement(LoopStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::NONBLOCKING_EVENT_TRIGGER_STATEMENT => {
                Some(Self::EventTriggerStatement(EventTriggerStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::CHECKER_INSTANCE_STATEMENT => Some(Self::CheckerInstanceStatement(
                CheckerInstanceStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::EMPTY_STATEMENT => {
                Some(Self::EmptyStatement(EmptyStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::VOID_CASTED_CALL_STATEMENT => {
                Some(Self::VoidCastedCallStatement(VoidCastedCallStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::RAND_SEQUENCE_STATEMENT => {
                Some(Self::RandSequenceStatement(RandSequenceStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::BLOCKING_EVENT_TRIGGER_STATEMENT => {
                Some(Self::EventTriggerStatement(EventTriggerStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::WAIT_FORK_STATEMENT => {
                Some(Self::WaitForkStatement(WaitForkStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::DISABLE_FORK_STATEMENT => {
                Some(Self::DisableForkStatement(DisableForkStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::TIMING_CONTROL_STATEMENT => {
                Some(Self::TimingControlStatement(TimingControlStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::RAND_CASE_STATEMENT => {
                Some(Self::RandCaseStatement(RandCaseStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::PARALLEL_BLOCK_STATEMENT => {
                Some(Self::BlockStatement(BlockStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::FOREVER_STATEMENT => {
                Some(Self::ForeverStatement(ForeverStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::CONDITIONAL_STATEMENT => {
                Some(Self::ConditionalStatement(ConditionalStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::IMMEDIATE_ASSUME_STATEMENT => Some(Self::ImmediateAssertionStatement(
                ImmediateAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::ASSUME_PROPERTY_STATEMENT => Some(Self::ConcurrentAssertionStatement(
                ConcurrentAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::IMMEDIATE_COVER_STATEMENT => Some(Self::ImmediateAssertionStatement(
                ImmediateAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::COVER_PROPERTY_STATEMENT => Some(Self::ConcurrentAssertionStatement(
                ConcurrentAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::ASSERT_PROPERTY_STATEMENT => Some(Self::ConcurrentAssertionStatement(
                ConcurrentAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::EXPECT_PROPERTY_STATEMENT => Some(Self::ConcurrentAssertionStatement(
                ConcurrentAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::DISABLE_STATEMENT => {
                Some(Self::DisableStatement(DisableStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT => {
                Some(Self::BlockStatement(BlockStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::IMMEDIATE_ASSERT_STATEMENT => Some(Self::ImmediateAssertionStatement(
                ImmediateAssertionStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::PROCEDURAL_ASSIGN_STATEMENT => Some(Self::ProceduralAssignStatement(
                ProceduralAssignStatement::cast(syntax).unwrap(),
            )),
            SyntaxKind::DO_WHILE_STATEMENT => {
                Some(Self::DoWhileStatement(DoWhileStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::CASE_STATEMENT => {
                Some(Self::CaseStatement(CaseStatement::cast(syntax).unwrap()))
            }
            SyntaxKind::FOR_LOOP_STATEMENT => {
                Some(Self::ForLoopStatement(ForLoopStatement::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimaryExpression<'a> {
    EmptyQueueExpression(EmptyQueueExpression<'a>),
    AssignmentPatternExpression(AssignmentPatternExpression<'a>),
    ConcatenationExpression(ConcatenationExpression<'a>),
    LiteralExpression(LiteralExpression<'a>),
    IntegerVectorExpression(IntegerVectorExpression<'a>),
    ParenthesizedExpression(ParenthesizedExpression<'a>),
    StreamingConcatenationExpression(StreamingConcatenationExpression<'a>),
    MultipleConcatenationExpression(MultipleConcatenationExpression<'a>),
}
impl<'a> PrimaryExpression<'a> {
    #[inline]
    pub fn as_empty_queue_expression(self) -> Option<EmptyQueueExpression<'a>> {
        match self {
            Self::EmptyQueueExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_assignment_pattern_expression(self) -> Option<AssignmentPatternExpression<'a>> {
        match self {
            Self::AssignmentPatternExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_concatenation_expression(self) -> Option<ConcatenationExpression<'a>> {
        match self {
            Self::ConcatenationExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_literal_expression(self) -> Option<LiteralExpression<'a>> {
        match self {
            Self::LiteralExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_integer_vector_expression(self) -> Option<IntegerVectorExpression<'a>> {
        match self {
            Self::IntegerVectorExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_parenthesized_expression(self) -> Option<ParenthesizedExpression<'a>> {
        match self {
            Self::ParenthesizedExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_streaming_concatenation_expression(
        self,
    ) -> Option<StreamingConcatenationExpression<'a>> {
        match self {
            Self::StreamingConcatenationExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_multiple_concatenation_expression(
        self,
    ) -> Option<MultipleConcatenationExpression<'a>> {
        match self {
            Self::MultipleConcatenationExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PrimaryExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::EmptyQueueExpression(node) => node.syntax(),
            Self::AssignmentPatternExpression(node) => node.syntax(),
            Self::ConcatenationExpression(node) => node.syntax(),
            Self::LiteralExpression(node) => node.syntax(),
            Self::IntegerVectorExpression(node) => node.syntax(),
            Self::ParenthesizedExpression(node) => node.syntax(),
            Self::StreamingConcatenationExpression(node) => node.syntax(),
            Self::MultipleConcatenationExpression(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_QUEUE_EXPRESSION
            || kind == SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION
            || kind == SyntaxKind::CONCATENATION_EXPRESSION
            || kind == SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION
            || kind == SyntaxKind::NULL_LITERAL_EXPRESSION
            || kind == SyntaxKind::TIME_LITERAL_EXPRESSION
            || kind == SyntaxKind::INTEGER_VECTOR_EXPRESSION
            || kind == SyntaxKind::REAL_LITERAL_EXPRESSION
            || kind == SyntaxKind::UNBASED_UNSIZED_LITERAL_EXPRESSION
            || kind == SyntaxKind::PARENTHESIZED_EXPRESSION
            || kind == SyntaxKind::STREAMING_CONCATENATION_EXPRESSION
            || kind == SyntaxKind::MULTIPLE_CONCATENATION_EXPRESSION
            || kind == SyntaxKind::WILDCARD_LITERAL_EXPRESSION
            || kind == SyntaxKind::STRING_LITERAL_EXPRESSION
            || kind == SyntaxKind::INTEGER_LITERAL_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::EMPTY_QUEUE_EXPRESSION => {
                Some(Self::EmptyQueueExpression(EmptyQueueExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION => Some(Self::AssignmentPatternExpression(
                AssignmentPatternExpression::cast(syntax).unwrap(),
            )),
            SyntaxKind::CONCATENATION_EXPRESSION => {
                Some(Self::ConcatenationExpression(ConcatenationExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION => {
                Some(Self::LiteralExpression(LiteralExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::NULL_LITERAL_EXPRESSION => {
                Some(Self::LiteralExpression(LiteralExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::TIME_LITERAL_EXPRESSION => {
                Some(Self::LiteralExpression(LiteralExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::INTEGER_VECTOR_EXPRESSION => {
                Some(Self::IntegerVectorExpression(IntegerVectorExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::REAL_LITERAL_EXPRESSION => {
                Some(Self::LiteralExpression(LiteralExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::UNBASED_UNSIZED_LITERAL_EXPRESSION => {
                Some(Self::LiteralExpression(LiteralExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::PARENTHESIZED_EXPRESSION => {
                Some(Self::ParenthesizedExpression(ParenthesizedExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::STREAMING_CONCATENATION_EXPRESSION => {
                Some(Self::StreamingConcatenationExpression(
                    StreamingConcatenationExpression::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::MULTIPLE_CONCATENATION_EXPRESSION => {
                Some(Self::MultipleConcatenationExpression(
                    MultipleConcatenationExpression::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::WILDCARD_LITERAL_EXPRESSION => {
                Some(Self::LiteralExpression(LiteralExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::STRING_LITERAL_EXPRESSION => {
                Some(Self::LiteralExpression(LiteralExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::INTEGER_LITERAL_EXPRESSION => {
                Some(Self::LiteralExpression(LiteralExpression::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModportNamedPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ModportNamedPort<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for ModportNamedPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_NAMED_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpEdgeField<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UdpEdgeField<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn first(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn second(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for UdpEdgeField<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_EDGE_FIELD
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConstraintBlock<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConstraintBlock<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, ConstraintItem<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ConstraintBlock<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONSTRAINT_BLOCK
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenthesizedConditionalDirectiveExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenthesizedConditionalDirectiveExpression<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn operand(&self) -> ConditionalDirectiveExpression<'a> {
        self.syntax().child_node(1usize).and_then(ConditionalDirectiveExpression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ParenthesizedConditionalDirectiveExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_CONDITIONAL_DIRECTIVE_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimingCheckEventCondition<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TimingCheckEventCondition<'a> {
    #[inline]
    pub fn triple_and(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for TimingCheckEventCondition<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TIMING_CHECK_EVENT_CONDITION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NameValuePragmaExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NameValuePragmaExpression<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn value(&self) -> PragmaExpression<'a> {
        self.syntax().child_node(2usize).and_then(PragmaExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for NameValuePragmaExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAME_VALUE_PRAGMA_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PragmaDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PragmaDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn args(&self) -> SeparatedList<'a, PragmaExpression<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for PragmaDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PRAGMA_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConfigDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConfigDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn config(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn semi_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn localparams(&self) -> SyntaxList<'a, ParameterDeclarationStatement<'a>> {
        self.syntax().child_node(4usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn design(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn top_cells(&self) -> SyntaxList<'a, ConfigCellIdentifier<'a>> {
        self.syntax().child_node(6usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn semi_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }

    #[inline]
    pub fn rules(&self) -> SyntaxList<'a, ConfigRule<'a>> {
        self.syntax().child_node(8usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endconfig(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(9usize)
    }

    #[inline]
    pub fn block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(10usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for ConfigDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONFIG_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OrderedParamAssignment<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> OrderedParamAssignment<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for OrderedParamAssignment<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ORDERED_PARAM_ASSIGNMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionalPredicate<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConditionalPredicate<'a> {
    #[inline]
    pub fn conditions(&self) -> SeparatedList<'a, ConditionalPattern<'a>> {
        self.syntax().child_node(0usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ConditionalPredicate<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONDITIONAL_PREDICATE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConcatenationExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConcatenationExpression<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expressions(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ConcatenationExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONCATENATION_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConstraintItem<'a> {
    LoopConstraint(LoopConstraint<'a>),
    UniquenessConstraint(UniquenessConstraint<'a>),
    ConstraintBlock(ConstraintBlock<'a>),
    ExpressionConstraint(ExpressionConstraint<'a>),
    DisableConstraint(DisableConstraint<'a>),
    ImplicationConstraint(ImplicationConstraint<'a>),
    SolveBeforeConstraint(SolveBeforeConstraint<'a>),
    ConditionalConstraint(ConditionalConstraint<'a>),
}
impl<'a> ConstraintItem<'a> {
    #[inline]
    pub fn as_loop_constraint(self) -> Option<LoopConstraint<'a>> {
        match self {
            Self::LoopConstraint(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_uniqueness_constraint(self) -> Option<UniquenessConstraint<'a>> {
        match self {
            Self::UniquenessConstraint(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_constraint_block(self) -> Option<ConstraintBlock<'a>> {
        match self {
            Self::ConstraintBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_expression_constraint(self) -> Option<ExpressionConstraint<'a>> {
        match self {
            Self::ExpressionConstraint(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_disable_constraint(self) -> Option<DisableConstraint<'a>> {
        match self {
            Self::DisableConstraint(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_implication_constraint(self) -> Option<ImplicationConstraint<'a>> {
        match self {
            Self::ImplicationConstraint(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_solve_before_constraint(self) -> Option<SolveBeforeConstraint<'a>> {
        match self {
            Self::SolveBeforeConstraint(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_conditional_constraint(self) -> Option<ConditionalConstraint<'a>> {
        match self {
            Self::ConditionalConstraint(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ConstraintItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::LoopConstraint(node) => node.syntax(),
            Self::UniquenessConstraint(node) => node.syntax(),
            Self::ConstraintBlock(node) => node.syntax(),
            Self::ExpressionConstraint(node) => node.syntax(),
            Self::DisableConstraint(node) => node.syntax(),
            Self::ImplicationConstraint(node) => node.syntax(),
            Self::SolveBeforeConstraint(node) => node.syntax(),
            Self::ConditionalConstraint(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LOOP_CONSTRAINT
            || kind == SyntaxKind::UNIQUENESS_CONSTRAINT
            || kind == SyntaxKind::CONSTRAINT_BLOCK
            || kind == SyntaxKind::EXPRESSION_CONSTRAINT
            || kind == SyntaxKind::DISABLE_CONSTRAINT
            || kind == SyntaxKind::IMPLICATION_CONSTRAINT
            || kind == SyntaxKind::SOLVE_BEFORE_CONSTRAINT
            || kind == SyntaxKind::CONDITIONAL_CONSTRAINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::LOOP_CONSTRAINT => {
                Some(Self::LoopConstraint(LoopConstraint::cast(syntax).unwrap()))
            }
            SyntaxKind::UNIQUENESS_CONSTRAINT => {
                Some(Self::UniquenessConstraint(UniquenessConstraint::cast(syntax).unwrap()))
            }
            SyntaxKind::CONSTRAINT_BLOCK => {
                Some(Self::ConstraintBlock(ConstraintBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::EXPRESSION_CONSTRAINT => {
                Some(Self::ExpressionConstraint(ExpressionConstraint::cast(syntax).unwrap()))
            }
            SyntaxKind::DISABLE_CONSTRAINT => {
                Some(Self::DisableConstraint(DisableConstraint::cast(syntax).unwrap()))
            }
            SyntaxKind::IMPLICATION_CONSTRAINT => {
                Some(Self::ImplicationConstraint(ImplicationConstraint::cast(syntax).unwrap()))
            }
            SyntaxKind::SOLVE_BEFORE_CONSTRAINT => {
                Some(Self::SolveBeforeConstraint(SolveBeforeConstraint::cast(syntax).unwrap()))
            }
            SyntaxKind::CONDITIONAL_CONSTRAINT => {
                Some(Self::ConditionalConstraint(ConditionalConstraint::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimpleSequenceExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SimpleSequenceExpr<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn repetition(&self) -> Option<SequenceRepetition<'a>> {
        self.syntax().child_node(1usize).and_then(SequenceRepetition::cast)
    }
}
impl<'a> AstNode<'a> for SimpleSequenceExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SIMPLE_SEQUENCE_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpBody<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UdpBody<'a> {
    #[inline]
    pub fn port_decls(&self) -> SeparatedList<'a, UdpPortDecl<'a>> {
        self.syntax().child_node(0usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn initial_stmt(&self) -> Option<UdpInitialStmt<'a>> {
        self.syntax().child_node(1usize).and_then(UdpInitialStmt::cast)
    }

    #[inline]
    pub fn table(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn entries(&self) -> SyntaxList<'a, UdpEntry<'a>> {
        self.syntax().child_node(3usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endtable(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for UdpBody<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_BODY
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacroActualArgument<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MacroActualArgument<'a> {
    #[inline]
    pub fn tokens(&self) -> TokenList<'a> {
        self.syntax().child_node(0usize).and_then(TokenList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for MacroActualArgument<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MACRO_ACTUAL_ARGUMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RandCaseItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RandCaseItem<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(2usize).and_then(Statement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for RandCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RAND_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Name<'a> {
    KeywordName(KeywordName<'a>),
    IdentifierSelectName(IdentifierSelectName<'a>),
    ClassName(ClassName<'a>),
    SystemName(SystemName<'a>),
    EmptyIdentifierName(EmptyIdentifierName<'a>),
    ScopedName(ScopedName<'a>),
    IdentifierName(IdentifierName<'a>),
}
impl<'a> Name<'a> {
    #[inline]
    pub fn as_keyword_name(self) -> Option<KeywordName<'a>> {
        match self {
            Self::KeywordName(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_identifier_select_name(self) -> Option<IdentifierSelectName<'a>> {
        match self {
            Self::IdentifierSelectName(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_class_name(self) -> Option<ClassName<'a>> {
        match self {
            Self::ClassName(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_system_name(self) -> Option<SystemName<'a>> {
        match self {
            Self::SystemName(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_empty_identifier_name(self) -> Option<EmptyIdentifierName<'a>> {
        match self {
            Self::EmptyIdentifierName(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_scoped_name(self) -> Option<ScopedName<'a>> {
        match self {
            Self::ScopedName(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_identifier_name(self) -> Option<IdentifierName<'a>> {
        match self {
            Self::IdentifierName(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Name<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::KeywordName(node) => node.syntax(),
            Self::IdentifierSelectName(node) => node.syntax(),
            Self::ClassName(node) => node.syntax(),
            Self::SystemName(node) => node.syntax(),
            Self::EmptyIdentifierName(node) => node.syntax(),
            Self::ScopedName(node) => node.syntax(),
            Self::IdentifierName(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LOCAL_SCOPE
            || kind == SyntaxKind::IDENTIFIER_SELECT_NAME
            || kind == SyntaxKind::CLASS_NAME
            || kind == SyntaxKind::ARRAY_OR_METHOD
            || kind == SyntaxKind::ARRAY_AND_METHOD
            || kind == SyntaxKind::ARRAY_UNIQUE_METHOD
            || kind == SyntaxKind::THIS_HANDLE
            || kind == SyntaxKind::SUPER_HANDLE
            || kind == SyntaxKind::CONSTRUCTOR_NAME
            || kind == SyntaxKind::SYSTEM_NAME
            || kind == SyntaxKind::ROOT_SCOPE
            || kind == SyntaxKind::UNIT_SCOPE
            || kind == SyntaxKind::EMPTY_IDENTIFIER_NAME
            || kind == SyntaxKind::ARRAY_XOR_METHOD
            || kind == SyntaxKind::SCOPED_NAME
            || kind == SyntaxKind::IDENTIFIER_NAME
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::LOCAL_SCOPE => Some(Self::KeywordName(KeywordName::cast(syntax).unwrap())),
            SyntaxKind::IDENTIFIER_SELECT_NAME => {
                Some(Self::IdentifierSelectName(IdentifierSelectName::cast(syntax).unwrap()))
            }
            SyntaxKind::CLASS_NAME => Some(Self::ClassName(ClassName::cast(syntax).unwrap())),
            SyntaxKind::ARRAY_OR_METHOD => {
                Some(Self::KeywordName(KeywordName::cast(syntax).unwrap()))
            }
            SyntaxKind::ARRAY_AND_METHOD => {
                Some(Self::KeywordName(KeywordName::cast(syntax).unwrap()))
            }
            SyntaxKind::ARRAY_UNIQUE_METHOD => {
                Some(Self::KeywordName(KeywordName::cast(syntax).unwrap()))
            }
            SyntaxKind::THIS_HANDLE => Some(Self::KeywordName(KeywordName::cast(syntax).unwrap())),
            SyntaxKind::SUPER_HANDLE => Some(Self::KeywordName(KeywordName::cast(syntax).unwrap())),
            SyntaxKind::CONSTRUCTOR_NAME => {
                Some(Self::KeywordName(KeywordName::cast(syntax).unwrap()))
            }
            SyntaxKind::SYSTEM_NAME => Some(Self::SystemName(SystemName::cast(syntax).unwrap())),
            SyntaxKind::ROOT_SCOPE => Some(Self::KeywordName(KeywordName::cast(syntax).unwrap())),
            SyntaxKind::UNIT_SCOPE => Some(Self::KeywordName(KeywordName::cast(syntax).unwrap())),
            SyntaxKind::EMPTY_IDENTIFIER_NAME => {
                Some(Self::EmptyIdentifierName(EmptyIdentifierName::cast(syntax).unwrap()))
            }
            SyntaxKind::ARRAY_XOR_METHOD => {
                Some(Self::KeywordName(KeywordName::cast(syntax).unwrap()))
            }
            SyntaxKind::SCOPED_NAME => Some(Self::ScopedName(ScopedName::cast(syntax).unwrap())),
            SyntaxKind::IDENTIFIER_NAME => {
                Some(Self::IdentifierName(IdentifierName::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenthesizedPattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenthesizedPattern<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn pattern(&self) -> Pattern<'a> {
        self.syntax().child_node(1usize).and_then(Pattern::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ParenthesizedPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARENTHESIZED_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LibraryIncludeStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LibraryIncludeStatement<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn include(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn file_path(&self) -> FilePathSpec<'a> {
        self.syntax().child_node(2usize).and_then(FilePathSpec::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for LibraryIncludeStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LIBRARY_INCLUDE_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BitSelect<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BitSelect<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for BitSelect<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BIT_SELECT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeywordType<'a> {
    ShortRealType(SyntaxNode<'a>),
    PropertyType(SyntaxNode<'a>),
    SequenceType(SyntaxNode<'a>),
    EventType(SyntaxNode<'a>),
    StringType(SyntaxNode<'a>),
    CHandleType(SyntaxNode<'a>),
    RealTimeType(SyntaxNode<'a>),
    VoidType(SyntaxNode<'a>),
    RealType(SyntaxNode<'a>),
    Untyped(SyntaxNode<'a>),
}
impl<'a> KeywordType<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn as_short_real_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ShortRealType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_property_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::PropertyType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_sequence_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::SequenceType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_event_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::EventType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_string_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::StringType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_c_handle_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::CHandleType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_real_time_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::RealTimeType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_void_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::VoidType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_real_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::RealType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_untyped(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::Untyped(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for KeywordType<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ShortRealType(node) => *node,
            Self::PropertyType(node) => *node,
            Self::SequenceType(node) => *node,
            Self::EventType(node) => *node,
            Self::StringType(node) => *node,
            Self::CHandleType(node) => *node,
            Self::RealTimeType(node) => *node,
            Self::VoidType(node) => *node,
            Self::RealType(node) => *node,
            Self::Untyped(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SHORT_REAL_TYPE
            || kind == SyntaxKind::PROPERTY_TYPE
            || kind == SyntaxKind::SEQUENCE_TYPE
            || kind == SyntaxKind::EVENT_TYPE
            || kind == SyntaxKind::STRING_TYPE
            || kind == SyntaxKind::C_HANDLE_TYPE
            || kind == SyntaxKind::REAL_TIME_TYPE
            || kind == SyntaxKind::VOID_TYPE
            || kind == SyntaxKind::REAL_TYPE
            || kind == SyntaxKind::UNTYPED
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::SHORT_REAL_TYPE => Some(Self::ShortRealType(syntax)),
            SyntaxKind::PROPERTY_TYPE => Some(Self::PropertyType(syntax)),
            SyntaxKind::SEQUENCE_TYPE => Some(Self::SequenceType(syntax)),
            SyntaxKind::EVENT_TYPE => Some(Self::EventType(syntax)),
            SyntaxKind::STRING_TYPE => Some(Self::StringType(syntax)),
            SyntaxKind::C_HANDLE_TYPE => Some(Self::CHandleType(syntax)),
            SyntaxKind::REAL_TIME_TYPE => Some(Self::RealTimeType(syntax)),
            SyntaxKind::VOID_TYPE => Some(Self::VoidType(syntax)),
            SyntaxKind::REAL_TYPE => Some(Self::RealType(syntax)),
            SyntaxKind::UNTYPED => Some(Self::Untyped(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModportDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ModportDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, ModportItem<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ModportDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MODPORT_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpInputPortDecl<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UdpInputPortDecl<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn names(&self) -> SeparatedList<'a, IdentifierName<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for UdpInputPortDecl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UDP_INPUT_PORT_DECL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConcurrentAssertionMember<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConcurrentAssertionMember<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn statement(&self) -> ConcurrentAssertionStatement<'a> {
        self.syntax().child_node(1usize).and_then(ConcurrentAssertionStatement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ConcurrentAssertionMember<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONCURRENT_ASSERTION_MEMBER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimingCheckArg<'a> {
    ExpressionTimingCheckArg(ExpressionTimingCheckArg<'a>),
    TimingCheckEventArg(TimingCheckEventArg<'a>),
    EmptyTimingCheckArg(EmptyTimingCheckArg<'a>),
}
impl<'a> TimingCheckArg<'a> {
    #[inline]
    pub fn as_expression_timing_check_arg(self) -> Option<ExpressionTimingCheckArg<'a>> {
        match self {
            Self::ExpressionTimingCheckArg(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_timing_check_event_arg(self) -> Option<TimingCheckEventArg<'a>> {
        match self {
            Self::TimingCheckEventArg(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_empty_timing_check_arg(self) -> Option<EmptyTimingCheckArg<'a>> {
        match self {
            Self::EmptyTimingCheckArg(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for TimingCheckArg<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ExpressionTimingCheckArg(node) => node.syntax(),
            Self::TimingCheckEventArg(node) => node.syntax(),
            Self::EmptyTimingCheckArg(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPRESSION_TIMING_CHECK_ARG
            || kind == SyntaxKind::TIMING_CHECK_EVENT_ARG
            || kind == SyntaxKind::EMPTY_TIMING_CHECK_ARG
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::EXPRESSION_TIMING_CHECK_ARG => Some(Self::ExpressionTimingCheckArg(
                ExpressionTimingCheckArg::cast(syntax).unwrap(),
            )),
            SyntaxKind::TIMING_CHECK_EVENT_ARG => {
                Some(Self::TimingCheckEventArg(TimingCheckEventArg::cast(syntax).unwrap()))
            }
            SyntaxKind::EMPTY_TIMING_CHECK_ARG => {
                Some(Self::EmptyTimingCheckArg(EmptyTimingCheckArg::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OneStepDelay<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> OneStepDelay<'a> {
    #[inline]
    pub fn hash(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn one_step(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for OneStepDelay<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ONE_STEP_DELAY
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DoWhileStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DoWhileStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn do_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(3usize).and_then(Statement::cast).unwrap()
    }

    #[inline]
    pub fn while_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(6usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }
}
impl<'a> AstNode<'a> for DoWhileStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DO_WHILE_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParameterDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParameterDeclaration<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(1usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, Declarator<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ParameterDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARAMETER_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StandardRsCaseItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> StandardRsCaseItem<'a> {
    #[inline]
    pub fn expressions(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(0usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn item(&self) -> RsProdItem<'a> {
        self.syntax().child_node(2usize).and_then(RsProdItem::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for StandardRsCaseItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STANDARD_RS_CASE_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PortHeader<'a> {
    VariablePortHeader(VariablePortHeader<'a>),
    NetPortHeader(NetPortHeader<'a>),
    InterfacePortHeader(InterfacePortHeader<'a>),
}
impl<'a> PortHeader<'a> {
    #[inline]
    pub fn as_variable_port_header(self) -> Option<VariablePortHeader<'a>> {
        match self {
            Self::VariablePortHeader(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_net_port_header(self) -> Option<NetPortHeader<'a>> {
        match self {
            Self::NetPortHeader(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_interface_port_header(self) -> Option<InterfacePortHeader<'a>> {
        match self {
            Self::InterfacePortHeader(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PortHeader<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::VariablePortHeader(node) => node.syntax(),
            Self::NetPortHeader(node) => node.syntax(),
            Self::InterfacePortHeader(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::VARIABLE_PORT_HEADER
            || kind == SyntaxKind::NET_PORT_HEADER
            || kind == SyntaxKind::INTERFACE_PORT_HEADER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::VARIABLE_PORT_HEADER => {
                Some(Self::VariablePortHeader(VariablePortHeader::cast(syntax).unwrap()))
            }
            SyntaxKind::NET_PORT_HEADER => {
                Some(Self::NetPortHeader(NetPortHeader::cast(syntax).unwrap()))
            }
            SyntaxKind::INTERFACE_PORT_HEADER => {
                Some(Self::InterfacePortHeader(InterfacePortHeader::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pattern<'a> {
    TaggedPattern(TaggedPattern<'a>),
    WildcardPattern(WildcardPattern<'a>),
    ExpressionPattern(ExpressionPattern<'a>),
    ParenthesizedPattern(ParenthesizedPattern<'a>),
    VariablePattern(VariablePattern<'a>),
    StructurePattern(StructurePattern<'a>),
}
impl<'a> Pattern<'a> {
    #[inline]
    pub fn as_tagged_pattern(self) -> Option<TaggedPattern<'a>> {
        match self {
            Self::TaggedPattern(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_wildcard_pattern(self) -> Option<WildcardPattern<'a>> {
        match self {
            Self::WildcardPattern(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_expression_pattern(self) -> Option<ExpressionPattern<'a>> {
        match self {
            Self::ExpressionPattern(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_parenthesized_pattern(self) -> Option<ParenthesizedPattern<'a>> {
        match self {
            Self::ParenthesizedPattern(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_variable_pattern(self) -> Option<VariablePattern<'a>> {
        match self {
            Self::VariablePattern(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_structure_pattern(self) -> Option<StructurePattern<'a>> {
        match self {
            Self::StructurePattern(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Pattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::TaggedPattern(node) => node.syntax(),
            Self::WildcardPattern(node) => node.syntax(),
            Self::ExpressionPattern(node) => node.syntax(),
            Self::ParenthesizedPattern(node) => node.syntax(),
            Self::VariablePattern(node) => node.syntax(),
            Self::StructurePattern(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TAGGED_PATTERN
            || kind == SyntaxKind::WILDCARD_PATTERN
            || kind == SyntaxKind::EXPRESSION_PATTERN
            || kind == SyntaxKind::PARENTHESIZED_PATTERN
            || kind == SyntaxKind::VARIABLE_PATTERN
            || kind == SyntaxKind::STRUCTURE_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::TAGGED_PATTERN => {
                Some(Self::TaggedPattern(TaggedPattern::cast(syntax).unwrap()))
            }
            SyntaxKind::WILDCARD_PATTERN => {
                Some(Self::WildcardPattern(WildcardPattern::cast(syntax).unwrap()))
            }
            SyntaxKind::EXPRESSION_PATTERN => {
                Some(Self::ExpressionPattern(ExpressionPattern::cast(syntax).unwrap()))
            }
            SyntaxKind::PARENTHESIZED_PATTERN => {
                Some(Self::ParenthesizedPattern(ParenthesizedPattern::cast(syntax).unwrap()))
            }
            SyntaxKind::VARIABLE_PATTERN => {
                Some(Self::VariablePattern(VariablePattern::cast(syntax).unwrap()))
            }
            SyntaxKind::STRUCTURE_PATTERN => {
                Some(Self::StructurePattern(StructurePattern::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RandJoinClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RandJoinClause<'a> {
    #[inline]
    pub fn rand(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn join(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<ParenthesizedExpression<'a>> {
        self.syntax().child_node(2usize).and_then(ParenthesizedExpression::cast)
    }
}
impl<'a> AstNode<'a> for RandJoinClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RAND_JOIN_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DistItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DistItem<'a> {
    #[inline]
    pub fn range(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn weight(&self) -> Option<DistWeight<'a>> {
        self.syntax().child_node(1usize).and_then(DistWeight::cast)
    }
}
impl<'a> AstNode<'a> for DistItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DIST_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockCoverageEvent<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BlockCoverageEvent<'a> {
    #[inline]
    pub fn atat(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> BlockEventExpression<'a> {
        self.syntax().child_node(2usize).and_then(BlockEventExpression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for BlockCoverageEvent<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BLOCK_COVERAGE_EVENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LoopGenerate<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LoopGenerate<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn genvar(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn initial_expr(&self) -> Expression<'a> {
        self.syntax().child_node(6usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi_1(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }

    #[inline]
    pub fn stop_expr(&self) -> Expression<'a> {
        self.syntax().child_node(8usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi_2(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(9usize)
    }

    #[inline]
    pub fn iteration_expr(&self) -> Expression<'a> {
        self.syntax().child_node(10usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(11usize)
    }

    #[inline]
    pub fn block(&self) -> Member<'a> {
        self.syntax().child_node(12usize).and_then(Member::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for LoopGenerate<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LOOP_GENERATE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GenerateBlock<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> GenerateBlock<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(1usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn begin(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn begin_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(3usize).and_then(NamedBlockClause::cast)
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(4usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn end_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(6usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for GenerateBlock<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::GENERATE_BLOCK
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinaryConditionalDirectiveExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BinaryConditionalDirectiveExpression<'a> {
    #[inline]
    pub fn left(&self) -> ConditionalDirectiveExpression<'a> {
        self.syntax().child_node(0usize).and_then(ConditionalDirectiveExpression::cast).unwrap()
    }

    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> ConditionalDirectiveExpression<'a> {
        self.syntax().child_node(2usize).and_then(ConditionalDirectiveExpression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for BinaryConditionalDirectiveExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BINARY_CONDITIONAL_DIRECTIVE_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeScaleDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TimeScaleDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn time_unit(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn slash(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn time_precision(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for TimeScaleDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TIME_SCALE_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigRuleClause<'a> {
    ConfigLiblist(ConfigLiblist<'a>),
    ConfigUseClause(ConfigUseClause<'a>),
}
impl<'a> ConfigRuleClause<'a> {
    #[inline]
    pub fn as_config_liblist(self) -> Option<ConfigLiblist<'a>> {
        match self {
            Self::ConfigLiblist(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_config_use_clause(self) -> Option<ConfigUseClause<'a>> {
        match self {
            Self::ConfigUseClause(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ConfigRuleClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ConfigLiblist(node) => node.syntax(),
            Self::ConfigUseClause(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONFIG_LIBLIST || kind == SyntaxKind::CONFIG_USE_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::CONFIG_LIBLIST => {
                Some(Self::ConfigLiblist(ConfigLiblist::cast(syntax).unwrap()))
            }
            SyntaxKind::CONFIG_USE_CLAUSE => {
                Some(Self::ConfigUseClause(ConfigUseClause::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Directive<'a> {
    PragmaDirective(PragmaDirective<'a>),
    MacroUsage(MacroUsage<'a>),
    SimpleDirective(SimpleDirective<'a>),
    LineDirective(LineDirective<'a>),
    UnconnectedDriveDirective(UnconnectedDriveDirective<'a>),
    UndefDirective(UndefDirective<'a>),
    ConditionalBranchDirective(ConditionalBranchDirective<'a>),
    IncludeDirective(IncludeDirective<'a>),
    BeginKeywordsDirective(BeginKeywordsDirective<'a>),
    DefaultDecayTimeDirective(DefaultDecayTimeDirective<'a>),
    UnconditionalBranchDirective(UnconditionalBranchDirective<'a>),
    DefaultTriregStrengthDirective(DefaultTriregStrengthDirective<'a>),
    DefaultNetTypeDirective(DefaultNetTypeDirective<'a>),
    TimeScaleDirective(TimeScaleDirective<'a>),
    DefineDirective(DefineDirective<'a>),
}
impl<'a> Directive<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn as_pragma_directive(self) -> Option<PragmaDirective<'a>> {
        match self {
            Self::PragmaDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_macro_usage(self) -> Option<MacroUsage<'a>> {
        match self {
            Self::MacroUsage(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_simple_directive(self) -> Option<SimpleDirective<'a>> {
        match self {
            Self::SimpleDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_line_directive(self) -> Option<LineDirective<'a>> {
        match self {
            Self::LineDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unconnected_drive_directive(self) -> Option<UnconnectedDriveDirective<'a>> {
        match self {
            Self::UnconnectedDriveDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_undef_directive(self) -> Option<UndefDirective<'a>> {
        match self {
            Self::UndefDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_conditional_branch_directive(self) -> Option<ConditionalBranchDirective<'a>> {
        match self {
            Self::ConditionalBranchDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_include_directive(self) -> Option<IncludeDirective<'a>> {
        match self {
            Self::IncludeDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_begin_keywords_directive(self) -> Option<BeginKeywordsDirective<'a>> {
        match self {
            Self::BeginKeywordsDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_decay_time_directive(self) -> Option<DefaultDecayTimeDirective<'a>> {
        match self {
            Self::DefaultDecayTimeDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unconditional_branch_directive(self) -> Option<UnconditionalBranchDirective<'a>> {
        match self {
            Self::UnconditionalBranchDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_trireg_strength_directive(
        self,
    ) -> Option<DefaultTriregStrengthDirective<'a>> {
        match self {
            Self::DefaultTriregStrengthDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_net_type_directive(self) -> Option<DefaultNetTypeDirective<'a>> {
        match self {
            Self::DefaultNetTypeDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_time_scale_directive(self) -> Option<TimeScaleDirective<'a>> {
        match self {
            Self::TimeScaleDirective(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_define_directive(self) -> Option<DefineDirective<'a>> {
        match self {
            Self::DefineDirective(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for Directive<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::PragmaDirective(node) => node.syntax(),
            Self::MacroUsage(node) => node.syntax(),
            Self::SimpleDirective(node) => node.syntax(),
            Self::LineDirective(node) => node.syntax(),
            Self::UnconnectedDriveDirective(node) => node.syntax(),
            Self::UndefDirective(node) => node.syntax(),
            Self::ConditionalBranchDirective(node) => node.syntax(),
            Self::IncludeDirective(node) => node.syntax(),
            Self::BeginKeywordsDirective(node) => node.syntax(),
            Self::DefaultDecayTimeDirective(node) => node.syntax(),
            Self::UnconditionalBranchDirective(node) => node.syntax(),
            Self::DefaultTriregStrengthDirective(node) => node.syntax(),
            Self::DefaultNetTypeDirective(node) => node.syntax(),
            Self::TimeScaleDirective(node) => node.syntax(),
            Self::DefineDirective(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PRAGMA_DIRECTIVE
            || kind == SyntaxKind::MACRO_USAGE
            || kind == SyntaxKind::RESET_ALL_DIRECTIVE
            || kind == SyntaxKind::LINE_DIRECTIVE
            || kind == SyntaxKind::CELL_DEFINE_DIRECTIVE
            || kind == SyntaxKind::END_CELL_DEFINE_DIRECTIVE
            || kind == SyntaxKind::UNCONNECTED_DRIVE_DIRECTIVE
            || kind == SyntaxKind::UNDEF_DIRECTIVE
            || kind == SyntaxKind::IF_N_DEF_DIRECTIVE
            || kind == SyntaxKind::INCLUDE_DIRECTIVE
            || kind == SyntaxKind::END_PROTECTED_DIRECTIVE
            || kind == SyntaxKind::UNDEFINE_ALL_DIRECTIVE
            || kind == SyntaxKind::PROTECTED_DIRECTIVE
            || kind == SyntaxKind::BEGIN_KEYWORDS_DIRECTIVE
            || kind == SyntaxKind::END_KEYWORDS_DIRECTIVE
            || kind == SyntaxKind::DEFAULT_DECAY_TIME_DIRECTIVE
            || kind == SyntaxKind::END_PROTECT_DIRECTIVE
            || kind == SyntaxKind::END_IF_DIRECTIVE
            || kind == SyntaxKind::DEFAULT_TRIREG_STRENGTH_DIRECTIVE
            || kind == SyntaxKind::ELSE_DIRECTIVE
            || kind == SyntaxKind::DELAY_MODE_PATH_DIRECTIVE
            || kind == SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE
            || kind == SyntaxKind::PROTECT_DIRECTIVE
            || kind == SyntaxKind::ELS_IF_DIRECTIVE
            || kind == SyntaxKind::IF_DEF_DIRECTIVE
            || kind == SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE
            || kind == SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE
            || kind == SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE
            || kind == SyntaxKind::TIME_SCALE_DIRECTIVE
            || kind == SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE
            || kind == SyntaxKind::DEFINE_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PRAGMA_DIRECTIVE => {
                Some(Self::PragmaDirective(PragmaDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::MACRO_USAGE => Some(Self::MacroUsage(MacroUsage::cast(syntax).unwrap())),
            SyntaxKind::RESET_ALL_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::LINE_DIRECTIVE => {
                Some(Self::LineDirective(LineDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::CELL_DEFINE_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::END_CELL_DEFINE_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::UNCONNECTED_DRIVE_DIRECTIVE => Some(Self::UnconnectedDriveDirective(
                UnconnectedDriveDirective::cast(syntax).unwrap(),
            )),
            SyntaxKind::UNDEF_DIRECTIVE => {
                Some(Self::UndefDirective(UndefDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::IF_N_DEF_DIRECTIVE => Some(Self::ConditionalBranchDirective(
                ConditionalBranchDirective::cast(syntax).unwrap(),
            )),
            SyntaxKind::INCLUDE_DIRECTIVE => {
                Some(Self::IncludeDirective(IncludeDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::END_PROTECTED_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::UNDEFINE_ALL_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::PROTECTED_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::BEGIN_KEYWORDS_DIRECTIVE => {
                Some(Self::BeginKeywordsDirective(BeginKeywordsDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::END_KEYWORDS_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_DECAY_TIME_DIRECTIVE => Some(Self::DefaultDecayTimeDirective(
                DefaultDecayTimeDirective::cast(syntax).unwrap(),
            )),
            SyntaxKind::END_PROTECT_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::END_IF_DIRECTIVE => Some(Self::UnconditionalBranchDirective(
                UnconditionalBranchDirective::cast(syntax).unwrap(),
            )),
            SyntaxKind::DEFAULT_TRIREG_STRENGTH_DIRECTIVE => {
                Some(Self::DefaultTriregStrengthDirective(
                    DefaultTriregStrengthDirective::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::ELSE_DIRECTIVE => Some(Self::UnconditionalBranchDirective(
                UnconditionalBranchDirective::cast(syntax).unwrap(),
            )),
            SyntaxKind::DELAY_MODE_PATH_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::PROTECT_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::ELS_IF_DIRECTIVE => Some(Self::ConditionalBranchDirective(
                ConditionalBranchDirective::cast(syntax).unwrap(),
            )),
            SyntaxKind::IF_DEF_DIRECTIVE => Some(Self::ConditionalBranchDirective(
                ConditionalBranchDirective::cast(syntax).unwrap(),
            )),
            SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE => {
                Some(Self::DefaultNetTypeDirective(DefaultNetTypeDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::TIME_SCALE_DIRECTIVE => {
                Some(Self::TimeScaleDirective(TimeScaleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE => {
                Some(Self::SimpleDirective(SimpleDirective::cast(syntax).unwrap()))
            }
            SyntaxKind::DEFINE_DIRECTIVE => {
                Some(Self::DefineDirective(DefineDirective::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NumberPragmaExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NumberPragmaExpression<'a> {
    #[inline]
    pub fn size(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn base(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn value(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for NumberPragmaExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NUMBER_PRAGMA_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssertionItemPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AssertionItemPort<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn local(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn direction(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(3usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn dimensions(&self) -> SyntaxList<'a, VariableDimension<'a>> {
        self.syntax().child_node(5usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn default_value(&self) -> Option<EqualsAssertionArgClause<'a>> {
        self.syntax().child_node(6usize).and_then(EqualsAssertionArgClause::cast)
    }
}
impl<'a> AstNode<'a> for AssertionItemPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ASSERTION_ITEM_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PullStrength<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PullStrength<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn strength(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for PullStrength<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PULL_STRENGTH
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Declarator<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> Declarator<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn dimensions(&self) -> SyntaxList<'a, VariableDimension<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn initializer(&self) -> Option<EqualsValueClause<'a>> {
        self.syntax().child_node(2usize).and_then(EqualsValueClause::cast)
    }
}
impl<'a> AstNode<'a> for Declarator<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DECLARATOR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverCross<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CoverCross<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(1usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn cross(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, IdentifierName<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn iff(&self) -> Option<CoverageIffClause<'a>> {
        self.syntax().child_node(4usize).and_then(CoverageIffClause::cast)
    }

    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(6usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }

    #[inline]
    pub fn empty_semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }
}
impl<'a> AstNode<'a> for CoverCross<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COVER_CROSS
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryPropertyExpr<'a> {
    AndPropertyExpr(SyntaxNode<'a>),
    IffPropertyExpr(SyntaxNode<'a>),
    ImplicationPropertyExpr(SyntaxNode<'a>),
    OrPropertyExpr(SyntaxNode<'a>),
    SUntilWithPropertyExpr(SyntaxNode<'a>),
    FollowedByPropertyExpr(SyntaxNode<'a>),
    ImpliesPropertyExpr(SyntaxNode<'a>),
    SUntilPropertyExpr(SyntaxNode<'a>),
    UntilPropertyExpr(SyntaxNode<'a>),
    UntilWithPropertyExpr(SyntaxNode<'a>),
}
impl<'a> BinaryPropertyExpr<'a> {
    #[inline]
    pub fn left(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(0usize).and_then(PropertyExpr::cast).unwrap()
    }

    #[inline]
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(2usize).and_then(PropertyExpr::cast).unwrap()
    }

    #[inline]
    pub fn as_and_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AndPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_iff_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::IffPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_implication_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ImplicationPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_or_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::OrPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_s_until_with_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::SUntilWithPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_followed_by_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::FollowedByPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_implies_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ImpliesPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_s_until_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::SUntilPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_until_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UntilPropertyExpr(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_until_with_property_expr(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UntilWithPropertyExpr(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for BinaryPropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::AndPropertyExpr(node) => *node,
            Self::IffPropertyExpr(node) => *node,
            Self::ImplicationPropertyExpr(node) => *node,
            Self::OrPropertyExpr(node) => *node,
            Self::SUntilWithPropertyExpr(node) => *node,
            Self::FollowedByPropertyExpr(node) => *node,
            Self::ImpliesPropertyExpr(node) => *node,
            Self::SUntilPropertyExpr(node) => *node,
            Self::UntilPropertyExpr(node) => *node,
            Self::UntilWithPropertyExpr(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::AND_PROPERTY_EXPR
            || kind == SyntaxKind::IFF_PROPERTY_EXPR
            || kind == SyntaxKind::IMPLICATION_PROPERTY_EXPR
            || kind == SyntaxKind::OR_PROPERTY_EXPR
            || kind == SyntaxKind::S_UNTIL_WITH_PROPERTY_EXPR
            || kind == SyntaxKind::FOLLOWED_BY_PROPERTY_EXPR
            || kind == SyntaxKind::IMPLIES_PROPERTY_EXPR
            || kind == SyntaxKind::S_UNTIL_PROPERTY_EXPR
            || kind == SyntaxKind::UNTIL_PROPERTY_EXPR
            || kind == SyntaxKind::UNTIL_WITH_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::AND_PROPERTY_EXPR => Some(Self::AndPropertyExpr(syntax)),
            SyntaxKind::IFF_PROPERTY_EXPR => Some(Self::IffPropertyExpr(syntax)),
            SyntaxKind::IMPLICATION_PROPERTY_EXPR => Some(Self::ImplicationPropertyExpr(syntax)),
            SyntaxKind::OR_PROPERTY_EXPR => Some(Self::OrPropertyExpr(syntax)),
            SyntaxKind::S_UNTIL_WITH_PROPERTY_EXPR => Some(Self::SUntilWithPropertyExpr(syntax)),
            SyntaxKind::FOLLOWED_BY_PROPERTY_EXPR => Some(Self::FollowedByPropertyExpr(syntax)),
            SyntaxKind::IMPLIES_PROPERTY_EXPR => Some(Self::ImpliesPropertyExpr(syntax)),
            SyntaxKind::S_UNTIL_PROPERTY_EXPR => Some(Self::SUntilPropertyExpr(syntax)),
            SyntaxKind::UNTIL_PROPERTY_EXPR => Some(Self::UntilPropertyExpr(syntax)),
            SyntaxKind::UNTIL_WITH_PROPERTY_EXPR => Some(Self::UntilWithPropertyExpr(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssignmentPatternItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AssignmentPatternItem<'a> {
    #[inline]
    pub fn key(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for AssignmentPatternItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ASSIGNMENT_PATTERN_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimingControlExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TimingControlExpression<'a> {
    #[inline]
    pub fn timing(&self) -> TimingControl<'a> {
        self.syntax().child_node(0usize).and_then(TimingControl::cast).unwrap()
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for TimingControlExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TIMING_CONTROL_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExplicitNonAnsiPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExplicitNonAnsiPort<'a> {
    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<PortExpression<'a>> {
        self.syntax().child_node(3usize).and_then(PortExpression::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for ExplicitNonAnsiPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPLICIT_NON_ANSI_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoverageBinInitializer<'a> {
    ExpressionCoverageBinInitializer(ExpressionCoverageBinInitializer<'a>),
    RangeCoverageBinInitializer(RangeCoverageBinInitializer<'a>),
    TransListCoverageBinInitializer(TransListCoverageBinInitializer<'a>),
    IdWithExprCoverageBinInitializer(IdWithExprCoverageBinInitializer<'a>),
    DefaultCoverageBinInitializer(DefaultCoverageBinInitializer<'a>),
}
impl<'a> CoverageBinInitializer<'a> {
    #[inline]
    pub fn as_expression_coverage_bin_initializer(
        self,
    ) -> Option<ExpressionCoverageBinInitializer<'a>> {
        match self {
            Self::ExpressionCoverageBinInitializer(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_range_coverage_bin_initializer(self) -> Option<RangeCoverageBinInitializer<'a>> {
        match self {
            Self::RangeCoverageBinInitializer(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_trans_list_coverage_bin_initializer(
        self,
    ) -> Option<TransListCoverageBinInitializer<'a>> {
        match self {
            Self::TransListCoverageBinInitializer(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_id_with_expr_coverage_bin_initializer(
        self,
    ) -> Option<IdWithExprCoverageBinInitializer<'a>> {
        match self {
            Self::IdWithExprCoverageBinInitializer(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_coverage_bin_initializer(self) -> Option<DefaultCoverageBinInitializer<'a>> {
        match self {
            Self::DefaultCoverageBinInitializer(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for CoverageBinInitializer<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ExpressionCoverageBinInitializer(node) => node.syntax(),
            Self::RangeCoverageBinInitializer(node) => node.syntax(),
            Self::TransListCoverageBinInitializer(node) => node.syntax(),
            Self::IdWithExprCoverageBinInitializer(node) => node.syntax(),
            Self::DefaultCoverageBinInitializer(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPRESSION_COVERAGE_BIN_INITIALIZER
            || kind == SyntaxKind::RANGE_COVERAGE_BIN_INITIALIZER
            || kind == SyntaxKind::TRANS_LIST_COVERAGE_BIN_INITIALIZER
            || kind == SyntaxKind::ID_WITH_EXPR_COVERAGE_BIN_INITIALIZER
            || kind == SyntaxKind::DEFAULT_COVERAGE_BIN_INITIALIZER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::EXPRESSION_COVERAGE_BIN_INITIALIZER => {
                Some(Self::ExpressionCoverageBinInitializer(
                    ExpressionCoverageBinInitializer::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::RANGE_COVERAGE_BIN_INITIALIZER => Some(Self::RangeCoverageBinInitializer(
                RangeCoverageBinInitializer::cast(syntax).unwrap(),
            )),
            SyntaxKind::TRANS_LIST_COVERAGE_BIN_INITIALIZER => {
                Some(Self::TransListCoverageBinInitializer(
                    TransListCoverageBinInitializer::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::ID_WITH_EXPR_COVERAGE_BIN_INITIALIZER => {
                Some(Self::IdWithExprCoverageBinInitializer(
                    IdWithExprCoverageBinInitializer::cast(syntax).unwrap(),
                ))
            }
            SyntaxKind::DEFAULT_COVERAGE_BIN_INITIALIZER => {
                Some(Self::DefaultCoverageBinInitializer(
                    DefaultCoverageBinInitializer::cast(syntax).unwrap(),
                ))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IncludeDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IncludeDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn file_name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for IncludeDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::INCLUDE_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdentifierName<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IdentifierName<'a> {
    #[inline]
    pub fn identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for IdentifierName<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IDENTIFIER_NAME
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DistConstraintList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DistConstraintList<'a> {
    #[inline]
    pub fn dist(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn items(&self) -> SeparatedList<'a, DistItemBase<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for DistConstraintList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DIST_CONSTRAINT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RsProd<'a> {
    RsCodeBlock(RsCodeBlock<'a>),
    RsRepeat(RsRepeat<'a>),
    RsIfElse(RsIfElse<'a>),
    RsProdItem(RsProdItem<'a>),
    RsCase(RsCase<'a>),
}
impl<'a> RsProd<'a> {
    #[inline]
    pub fn as_rs_code_block(self) -> Option<RsCodeBlock<'a>> {
        match self {
            Self::RsCodeBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_rs_repeat(self) -> Option<RsRepeat<'a>> {
        match self {
            Self::RsRepeat(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_rs_if_else(self) -> Option<RsIfElse<'a>> {
        match self {
            Self::RsIfElse(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_rs_prod_item(self) -> Option<RsProdItem<'a>> {
        match self {
            Self::RsProdItem(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_rs_case(self) -> Option<RsCase<'a>> {
        match self {
            Self::RsCase(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for RsProd<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::RsCodeBlock(node) => node.syntax(),
            Self::RsRepeat(node) => node.syntax(),
            Self::RsIfElse(node) => node.syntax(),
            Self::RsProdItem(node) => node.syntax(),
            Self::RsCase(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_CODE_BLOCK
            || kind == SyntaxKind::RS_REPEAT
            || kind == SyntaxKind::RS_IF_ELSE
            || kind == SyntaxKind::RS_PROD_ITEM
            || kind == SyntaxKind::RS_CASE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::RS_CODE_BLOCK => {
                Some(Self::RsCodeBlock(RsCodeBlock::cast(syntax).unwrap()))
            }
            SyntaxKind::RS_REPEAT => Some(Self::RsRepeat(RsRepeat::cast(syntax).unwrap())),
            SyntaxKind::RS_IF_ELSE => Some(Self::RsIfElse(RsIfElse::cast(syntax).unwrap())),
            SyntaxKind::RS_PROD_ITEM => Some(Self::RsProdItem(RsProdItem::cast(syntax).unwrap())),
            SyntaxKind::RS_CASE => Some(Self::RsCase(RsCase::cast(syntax).unwrap())),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VariablePattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> VariablePattern<'a> {
    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn variable_name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for VariablePattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::VARIABLE_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyArgument<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EmptyArgument<'a> {
    #[inline]
    pub fn placeholder(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for EmptyArgument<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_ARGUMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IffEventClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> IffEventClause<'a> {
    #[inline]
    pub fn iff(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for IffEventClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IFF_EVENT_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DataDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DataDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn modifiers(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(2usize).and_then(DataType::cast).unwrap()
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, Declarator<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for DataDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DATA_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RepeatedEventControl<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RepeatedEventControl<'a> {
    #[inline]
    pub fn repeat(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn event_control(&self) -> Option<TimingControl<'a>> {
        self.syntax().child_node(4usize).and_then(TimingControl::cast)
    }
}
impl<'a> AstNode<'a> for RepeatedEventControl<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::REPEATED_EVENT_CONTROL
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColonExpressionClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ColonExpressionClause<'a> {
    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ColonExpressionClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COLON_EXPRESSION_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsCodeBlock<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> RsCodeBlock<'a> {
    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, HybridNode<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for RsCodeBlock<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::RS_CODE_BLOCK
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockingDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClockingDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn global_or_default(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn clocking(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn block_name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn at(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn event(&self) -> EventExpression<'a> {
        self.syntax().child_node(5usize).and_then(EventExpression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(7usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end_clocking(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(8usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(9usize).and_then(NamedBlockClause::cast)
    }
}
impl<'a> AstNode<'a> for ClockingDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLOCKING_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultClockingReference<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultClockingReference<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn clocking(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for DefaultClockingReference<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_CLOCKING_REFERENCE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElabSystemTask<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ElabSystemTask<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn arguments(&self) -> Option<ArgumentList<'a>> {
        self.syntax().child_node(2usize).and_then(ArgumentList::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for ElabSystemTask<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ELAB_SYSTEM_TASK
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FirstMatchSequenceExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> FirstMatchSequenceExpr<'a> {
    #[inline]
    pub fn first_match(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> SequenceExpr<'a> {
        self.syntax().child_node(2usize).and_then(SequenceExpr::cast).unwrap()
    }

    #[inline]
    pub fn match_list(&self) -> Option<SequenceMatchList<'a>> {
        self.syntax().child_node(3usize).and_then(SequenceMatchList::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for FirstMatchSequenceExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FIRST_MATCH_SEQUENCE_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FunctionPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> FunctionPortList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ports(&self) -> SeparatedList<'a, FunctionPortBase<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for FunctionPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::FUNCTION_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProceduralBlock<'a> {
    AlwaysCombBlock(SyntaxNode<'a>),
    InitialBlock(SyntaxNode<'a>),
    AlwaysFFBlock(SyntaxNode<'a>),
    AlwaysLatchBlock(SyntaxNode<'a>),
    AlwaysBlock(SyntaxNode<'a>),
    FinalBlock(SyntaxNode<'a>),
}
impl<'a> ProceduralBlock<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(2usize).and_then(Statement::cast).unwrap()
    }

    #[inline]
    pub fn as_always_comb_block(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AlwaysCombBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_initial_block(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::InitialBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_always_ff_block(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AlwaysFFBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_always_latch_block(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AlwaysLatchBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_always_block(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::AlwaysBlock(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_final_block(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::FinalBlock(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for ProceduralBlock<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::AlwaysCombBlock(node) => *node,
            Self::InitialBlock(node) => *node,
            Self::AlwaysFFBlock(node) => *node,
            Self::AlwaysLatchBlock(node) => *node,
            Self::AlwaysBlock(node) => *node,
            Self::FinalBlock(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ALWAYS_COMB_BLOCK
            || kind == SyntaxKind::INITIAL_BLOCK
            || kind == SyntaxKind::ALWAYS_FF_BLOCK
            || kind == SyntaxKind::ALWAYS_LATCH_BLOCK
            || kind == SyntaxKind::ALWAYS_BLOCK
            || kind == SyntaxKind::FINAL_BLOCK
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::ALWAYS_COMB_BLOCK => Some(Self::AlwaysCombBlock(syntax)),
            SyntaxKind::INITIAL_BLOCK => Some(Self::InitialBlock(syntax)),
            SyntaxKind::ALWAYS_FF_BLOCK => Some(Self::AlwaysFFBlock(syntax)),
            SyntaxKind::ALWAYS_LATCH_BLOCK => Some(Self::AlwaysLatchBlock(syntax)),
            SyntaxKind::ALWAYS_BLOCK => Some(Self::AlwaysBlock(syntax)),
            SyntaxKind::FINAL_BLOCK => Some(Self::FinalBlock(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortReference<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PortReference<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn select(&self) -> Option<ElementSelect<'a>> {
        self.syntax().child_node(1usize).and_then(ElementSelect::cast)
    }
}
impl<'a> AstNode<'a> for PortReference<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PORT_REFERENCE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WithFunctionSample<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WithFunctionSample<'a> {
    #[inline]
    pub fn with(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn function(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn sample(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn port_list(&self) -> Option<FunctionPortList<'a>> {
        self.syntax().child_node(3usize).and_then(FunctionPortList::cast)
    }
}
impl<'a> AstNode<'a> for WithFunctionSample<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WITH_FUNCTION_SAMPLE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StructUnionType<'a> {
    StructType(SyntaxNode<'a>),
    UnionType(SyntaxNode<'a>),
}
impl<'a> StructUnionType<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn tagged_or_soft(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn packed(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn signing(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn open_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, StructUnionMember<'a>> {
        self.syntax().child_node(5usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn close_brace(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn dimensions(&self) -> SyntaxList<'a, VariableDimension<'a>> {
        self.syntax().child_node(7usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn as_struct_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::StructType(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_union_type(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::UnionType(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for StructUnionType<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::StructType(node) => *node,
            Self::UnionType(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::STRUCT_TYPE || kind == SyntaxKind::UNION_TYPE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::STRUCT_TYPE => Some(Self::StructType(syntax)),
            SyntaxKind::UNION_TYPE => Some(Self::UnionType(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImmediateAssertionMember<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ImmediateAssertionMember<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn statement(&self) -> ImmediateAssertionStatement<'a> {
        self.syntax().child_node(1usize).and_then(ImmediateAssertionStatement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ImmediateAssertionMember<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::IMMEDIATE_ASSERTION_MEMBER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CaseGenerate<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CaseGenerate<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn condition(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, CaseItem<'a>> {
        self.syntax().child_node(5usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end_case(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }
}
impl<'a> AstNode<'a> for CaseGenerate<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CASE_GENERATE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinsSelectConditionExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BinsSelectConditionExpr<'a> {
    #[inline]
    pub fn binsof(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(2usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn intersects(&self) -> Option<IntersectClause<'a>> {
        self.syntax().child_node(4usize).and_then(IntersectClause::cast)
    }
}
impl<'a> AstNode<'a> for BinsSelectConditionExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BINS_SELECT_CONDITION_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenExpressionList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenExpressionList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn expressions(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ParenExpressionList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PAREN_EXPRESSION_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacroArgumentDefault<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> MacroArgumentDefault<'a> {
    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn tokens(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for MacroArgumentDefault<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::MACRO_ARGUMENT_DEFAULT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DPIExport<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DPIExport<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn spec_string(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn c_identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn function_or_task(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(7usize)
    }
}
impl<'a> AstNode<'a> for DPIExport<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DPI_EXPORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UndefDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> UndefDirective<'a> {
    #[inline]
    pub fn directive(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for UndefDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::UNDEF_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PortDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn header(&self) -> PortHeader<'a> {
        self.syntax().child_node(1usize).and_then(PortHeader::cast).unwrap()
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, Declarator<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for PortDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PORT_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpecparamDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SpecparamDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn type_(&self) -> ImplicitType<'a> {
        self.syntax().child_node(2usize).and_then(ImplicitType::cast).unwrap()
    }

    #[inline]
    pub fn declarators(&self) -> SeparatedList<'a, SpecparamDeclarator<'a>> {
        self.syntax().child_node(3usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for SpecparamDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SPECPARAM_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PragmaExpression<'a> {
    NameValuePragmaExpression(NameValuePragmaExpression<'a>),
    ParenPragmaExpression(ParenPragmaExpression<'a>),
    SimplePragmaExpression(SimplePragmaExpression<'a>),
    NumberPragmaExpression(NumberPragmaExpression<'a>),
}
impl<'a> PragmaExpression<'a> {
    #[inline]
    pub fn as_name_value_pragma_expression(self) -> Option<NameValuePragmaExpression<'a>> {
        match self {
            Self::NameValuePragmaExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_paren_pragma_expression(self) -> Option<ParenPragmaExpression<'a>> {
        match self {
            Self::ParenPragmaExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_simple_pragma_expression(self) -> Option<SimplePragmaExpression<'a>> {
        match self {
            Self::SimplePragmaExpression(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_number_pragma_expression(self) -> Option<NumberPragmaExpression<'a>> {
        match self {
            Self::NumberPragmaExpression(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for PragmaExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::NameValuePragmaExpression(node) => node.syntax(),
            Self::ParenPragmaExpression(node) => node.syntax(),
            Self::SimplePragmaExpression(node) => node.syntax(),
            Self::NumberPragmaExpression(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAME_VALUE_PRAGMA_EXPRESSION
            || kind == SyntaxKind::PAREN_PRAGMA_EXPRESSION
            || kind == SyntaxKind::SIMPLE_PRAGMA_EXPRESSION
            || kind == SyntaxKind::NUMBER_PRAGMA_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::NAME_VALUE_PRAGMA_EXPRESSION => Some(Self::NameValuePragmaExpression(
                NameValuePragmaExpression::cast(syntax).unwrap(),
            )),
            SyntaxKind::PAREN_PRAGMA_EXPRESSION => {
                Some(Self::ParenPragmaExpression(ParenPragmaExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::SIMPLE_PRAGMA_EXPRESSION => {
                Some(Self::SimplePragmaExpression(SimplePragmaExpression::cast(syntax).unwrap()))
            }
            SyntaxKind::NUMBER_PRAGMA_EXPRESSION => {
                Some(Self::NumberPragmaExpression(NumberPragmaExpression::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QueueDimensionSpecifier<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> QueueDimensionSpecifier<'a> {
    #[inline]
    pub fn dollar(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn max_size_clause(&self) -> Option<ColonExpressionClause<'a>> {
        self.syntax().child_node(1usize).and_then(ColonExpressionClause::cast)
    }
}
impl<'a> AstNode<'a> for QueueDimensionSpecifier<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::QUEUE_DIMENSION_SPECIFIER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementSelect<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ElementSelect<'a> {
    #[inline]
    pub fn open_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn selector(&self) -> Option<Selector<'a>> {
        self.syntax().child_node(1usize).and_then(Selector::cast)
    }

    #[inline]
    pub fn close_bracket(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ElementSelect<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ELEMENT_SELECT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopedName<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ScopedName<'a> {
    #[inline]
    pub fn left(&self) -> Name<'a> {
        self.syntax().child_node(0usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn separator(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn right(&self) -> Name<'a> {
        self.syntax().child_node(2usize).and_then(Name::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ScopedName<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SCOPED_NAME
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChargeStrength<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ChargeStrength<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn strength(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ChargeStrength<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CHARGE_STRENGTH
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedBlockClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NamedBlockClause<'a> {
    #[inline]
    pub fn colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for NamedBlockClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NAMED_BLOCK_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContinuousAssign<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ContinuousAssign<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn assign(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn strength(&self) -> Option<DriveStrength<'a>> {
        self.syntax().child_node(2usize).and_then(DriveStrength::cast)
    }

    #[inline]
    pub fn delay(&self) -> Option<TimingControl<'a>> {
        self.syntax().child_node(3usize).and_then(TimingControl::cast)
    }

    #[inline]
    pub fn assignments(&self) -> SeparatedList<'a, Expression<'a>> {
        self.syntax().child_node(4usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for ContinuousAssign<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONTINUOUS_ASSIGN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParenPragmaExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ParenPragmaExpression<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn values(&self) -> SeparatedList<'a, PragmaExpression<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for ParenPragmaExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PAREN_PRAGMA_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimingControlStatement<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TimingControlStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn timing_control(&self) -> TimingControl<'a> {
        self.syntax().child_node(2usize).and_then(TimingControl::cast).unwrap()
    }

    #[inline]
    pub fn statement(&self) -> Statement<'a> {
        self.syntax().child_node(3usize).and_then(Statement::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for TimingControlStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TIMING_CONTROL_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DistItemBase<'a> {
    DistItem(DistItem<'a>),
    DefaultDistItem(DefaultDistItem<'a>),
}
impl<'a> DistItemBase<'a> {
    #[inline]
    pub fn as_dist_item(self) -> Option<DistItem<'a>> {
        match self {
            Self::DistItem(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_default_dist_item(self) -> Option<DefaultDistItem<'a>> {
        match self {
            Self::DefaultDistItem(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for DistItemBase<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::DistItem(node) => node.syntax(),
            Self::DefaultDistItem(node) => node.syntax(),
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DIST_ITEM || kind == SyntaxKind::DEFAULT_DIST_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::DIST_ITEM => Some(Self::DistItem(DistItem::cast(syntax).unwrap())),
            SyntaxKind::DEFAULT_DIST_ITEM => {
                Some(Self::DefaultDistItem(DefaultDistItem::cast(syntax).unwrap()))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GenvarDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> GenvarDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn identifiers(&self) -> SeparatedList<'a, IdentifierName<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }
}
impl<'a> AstNode<'a> for GenvarDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::GENVAR_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageImportItem<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PackageImportItem<'a> {
    #[inline]
    pub fn package(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn double_colon(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn item(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for PackageImportItem<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PACKAGE_IMPORT_ITEM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindDirective<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> BindDirective<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn bind(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn target(&self) -> Name<'a> {
        self.syntax().child_node(2usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn target_instances(&self) -> Option<BindTargetList<'a>> {
        self.syntax().child_node(3usize).and_then(BindTargetList::cast)
    }

    #[inline]
    pub fn instantiation(&self) -> Member<'a> {
        self.syntax().child_node(4usize).and_then(Member::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for BindDirective<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::BIND_DIRECTIVE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LoopConstraint<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> LoopConstraint<'a> {
    #[inline]
    pub fn foreach_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn loop_list(&self) -> ForeachLoopList<'a> {
        self.syntax().child_node(1usize).and_then(ForeachLoopList::cast).unwrap()
    }

    #[inline]
    pub fn constraints(&self) -> ConstraintItem<'a> {
        self.syntax().child_node(2usize).and_then(ConstraintItem::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for LoopConstraint<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::LOOP_CONSTRAINT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionCoverageBinInitializer<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExpressionCoverageBinInitializer<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ExpressionCoverageBinInitializer<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPRESSION_COVERAGE_BIN_INITIALIZER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyIdentifierName<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EmptyIdentifierName<'a> {
    #[inline]
    pub fn placeholder(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for EmptyIdentifierName<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EMPTY_IDENTIFIER_NAME
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeInstance<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AttributeInstance<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_star(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn specs(&self) -> SeparatedList<'a, AttributeSpec<'a>> {
        self.syntax().child_node(2usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_star(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for AttributeInstance<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ATTRIBUTE_INSTANCE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultDisableDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultDisableDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn disable_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn iff_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(4usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for DefaultDisableDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_DISABLE_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PrimaryBlockEventExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> PrimaryBlockEventExpression<'a> {
    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(1usize).and_then(Name::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for PrimaryBlockEventExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PRIMARY_BLOCK_EVENT_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompilationUnit<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CompilationUnit<'a> {
    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end_of_file(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }
}
impl<'a> AstNode<'a> for CompilationUnit<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COMPILATION_UNIT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassPropertyDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClassPropertyDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn qualifiers(&self) -> TokenList<'a> {
        self.syntax().child_node(1usize).and_then(TokenList::cast).unwrap()
    }

    #[inline]
    pub fn declaration(&self) -> Member<'a> {
        self.syntax().child_node(2usize).and_then(Member::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ClassPropertyDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLASS_PROPERTY_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CasePropertyExpr<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CasePropertyExpr<'a> {
    #[inline]
    pub fn case_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(2usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, PropertyCaseItem<'a>> {
        self.syntax().child_node(4usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endcase(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }
}
impl<'a> AstNode<'a> for CasePropertyExpr<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CASE_PROPERTY_EXPR
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NonAnsiPortList<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> NonAnsiPortList<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn ports(&self) -> SeparatedList<'a, NonAnsiPort<'a>> {
        self.syntax().child_node(1usize).and_then(SeparatedList::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for NonAnsiPortList<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::NON_ANSI_PORT_LIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExplicitAnsiPort<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExplicitAnsiPort<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn direction(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn dot(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(3usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }

    #[inline]
    pub fn expr(&self) -> Option<Expression<'a>> {
        self.syntax().child_node(5usize).and_then(Expression::cast)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(6usize)
    }
}
impl<'a> AstNode<'a> for ExplicitAnsiPort<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPLICIT_ANSI_PORT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockStatement<'a> {
    ParallelBlockStatement(SyntaxNode<'a>),
    SequentialBlockStatement(SyntaxNode<'a>),
}
impl<'a> BlockStatement<'a> {
    #[inline]
    pub fn label(&self) -> Option<NamedLabel<'a>> {
        self.syntax().child_node(0usize).and_then(NamedLabel::cast)
    }

    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(1usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn begin(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(3usize).and_then(NamedBlockClause::cast)
    }

    #[inline]
    pub fn items(&self) -> SyntaxList<'a, HybridNode<'a>> {
        self.syntax().child_node(4usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn end(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(5usize)
    }

    #[inline]
    pub fn end_block_name(&self) -> Option<NamedBlockClause<'a>> {
        self.syntax().child_node(6usize).and_then(NamedBlockClause::cast)
    }

    #[inline]
    pub fn as_parallel_block_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::ParallelBlockStatement(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn as_sequential_block_statement(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::SequentialBlockStatement(node) => Some(node),
            _ => None,
        }
    }
}
impl<'a> AstNode<'a> for BlockStatement<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        match self {
            Self::ParallelBlockStatement(node) => *node,
            Self::SequentialBlockStatement(node) => *node,
        }
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PARALLEL_BLOCK_STATEMENT
            || kind == SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PARALLEL_BLOCK_STATEMENT => Some(Self::ParallelBlockStatement(syntax)),
            SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT => Some(Self::SequentialBlockStatement(syntax)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OrderedPortConnection<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> OrderedPortConnection<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn expr(&self) -> PropertyExpr<'a> {
        self.syntax().child_node(1usize).and_then(PropertyExpr::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for OrderedPortConnection<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ORDERED_PORT_CONNECTION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnonymousProgram<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> AnonymousProgram<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, Member<'a>> {
        self.syntax().child_node(3usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn endkeyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for AnonymousProgram<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ANONYMOUS_PROGRAM
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultExtendsClauseArg<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DefaultExtendsClauseArg<'a> {
    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn default_keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }
}
impl<'a> AstNode<'a> for DefaultExtendsClauseArg<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DEFAULT_EXTENDS_CLAUSE_ARG
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionOrDist<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ExpressionOrDist<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn distribution(&self) -> DistConstraintList<'a> {
        self.syntax().child_node(1usize).and_then(DistConstraintList::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ExpressionOrDist<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EXPRESSION_OR_DIST
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeUnitsDeclaration<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> TimeUnitsDeclaration<'a> {
    #[inline]
    pub fn attributes(&self) -> SyntaxList<'a, AttributeInstance<'a>> {
        self.syntax().child_node(0usize).and_then(SyntaxList::cast).unwrap()
    }

    #[inline]
    pub fn keyword(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn time(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn divider(&self) -> Option<DividerClause<'a>> {
        self.syntax().child_node(3usize).and_then(DividerClause::cast)
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for TimeUnitsDeclaration<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TIME_UNITS_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisableIff<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> DisableIff<'a> {
    #[inline]
    pub fn disable(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn iff(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(1usize)
    }

    #[inline]
    pub fn open_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(2usize)
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(3usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn close_paren(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(4usize)
    }
}
impl<'a> AstNode<'a> for DisableIff<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::DISABLE_IFF
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SystemName<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> SystemName<'a> {
    #[inline]
    pub fn system_identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }
}
impl<'a> AstNode<'a> for SystemName<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SYSTEM_NAME
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClassName<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ClassName<'a> {
    #[inline]
    pub fn identifier(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn parameters(&self) -> ParameterValueAssignment<'a> {
        self.syntax().child_node(1usize).and_then(ParameterValueAssignment::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for ClassName<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CLASS_NAME
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConditionalPattern<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> ConditionalPattern<'a> {
    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(0usize).and_then(Expression::cast).unwrap()
    }

    #[inline]
    pub fn matches_clause(&self) -> Option<MatchesClause<'a>> {
        self.syntax().child_node(1usize).and_then(MatchesClause::cast)
    }
}
impl<'a> AstNode<'a> for ConditionalPattern<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::CONDITIONAL_PATTERN
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EqualsTypeClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> EqualsTypeClause<'a> {
    #[inline]
    pub fn equals(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn type_(&self) -> DataType<'a> {
        self.syntax().child_node(1usize).and_then(DataType::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for EqualsTypeClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::EQUALS_TYPE_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CopyClassExpression<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> CopyClassExpression<'a> {
    #[inline]
    pub fn scoped_new(&self) -> Name<'a> {
        self.syntax().child_node(0usize).and_then(Name::cast).unwrap()
    }

    #[inline]
    pub fn expr(&self) -> Expression<'a> {
        self.syntax().child_node(1usize).and_then(Expression::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for CopyClassExpression<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::COPY_CLASS_EXPRESSION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WithFunctionClause<'a> {
    syntax: SyntaxNode<'a>,
}
impl<'a> WithFunctionClause<'a> {
    #[inline]
    pub fn with(&self) -> Option<SyntaxToken<'a>> {
        self.syntax().child_token(0usize)
    }

    #[inline]
    pub fn name(&self) -> Name<'a> {
        self.syntax().child_node(1usize).and_then(Name::cast).unwrap()
    }
}
impl<'a> AstNode<'a> for WithFunctionClause<'a> {
    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }

    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::WITH_FUNCTION_CLAUSE
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }
}
