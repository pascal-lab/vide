use rowan::{GreenNode, Language, NodeOrToken};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    Root = 0,
    UnknownNode,
    UnknownToken,
    UnknownTrivia,
    Whitespace,
    Comment,
    EndOfLine,
    CompilationUnit,
    SyntaxList,
    SeparatedList,
    ModuleDeclaration,
    ModuleHeader,
    AnsiPortList,
    ImplicitAnsiPort,
    VariablePortHeader,
    ImplicitType,
    Placeholder,
    DataDeclaration,
    NetType,
    Declarator,
    ContinuousAssign,
    AssignmentExpression,
    NamedValueExpression,
    Identifier,
    SystemIdentifier,
    StringLiteral,
    IntegerLiteral,
    RealLiteral,
    TimeLiteral,
    ModuleKeyword,
    EndModuleKeyword,
    InputKeyword,
    OutputKeyword,
    InOutKeyword,
    WireKeyword,
    LogicKeyword,
    AssignKeyword,
    Semicolon,
    Colon,
    Comma,
    Dot,
    Hash,
    Equals,
    Minus,
    OpenParenthesis,
    CloseParenthesis,
    OpenBracket,
    CloseBracket,
}

impl SyntaxKind {
    pub const fn is_unknown(self) -> bool {
        matches!(self, Self::UnknownNode | Self::UnknownToken | Self::UnknownTrivia)
    }

    fn from_raw(raw: rowan::SyntaxKind) -> Self {
        match raw.0 {
            0 => Self::Root,
            1 => Self::UnknownNode,
            2 => Self::UnknownToken,
            3 => Self::UnknownTrivia,
            4 => Self::Whitespace,
            5 => Self::Comment,
            6 => Self::EndOfLine,
            7 => Self::CompilationUnit,
            8 => Self::SyntaxList,
            9 => Self::SeparatedList,
            10 => Self::ModuleDeclaration,
            11 => Self::ModuleHeader,
            12 => Self::AnsiPortList,
            13 => Self::ImplicitAnsiPort,
            14 => Self::VariablePortHeader,
            15 => Self::ImplicitType,
            16 => Self::Placeholder,
            17 => Self::DataDeclaration,
            18 => Self::NetType,
            19 => Self::Declarator,
            20 => Self::ContinuousAssign,
            21 => Self::AssignmentExpression,
            22 => Self::NamedValueExpression,
            23 => Self::Identifier,
            24 => Self::SystemIdentifier,
            25 => Self::StringLiteral,
            26 => Self::IntegerLiteral,
            27 => Self::RealLiteral,
            28 => Self::TimeLiteral,
            29 => Self::ModuleKeyword,
            30 => Self::EndModuleKeyword,
            31 => Self::InputKeyword,
            32 => Self::OutputKeyword,
            33 => Self::InOutKeyword,
            34 => Self::WireKeyword,
            35 => Self::LogicKeyword,
            36 => Self::AssignKeyword,
            37 => Self::Semicolon,
            38 => Self::Colon,
            39 => Self::Comma,
            40 => Self::Dot,
            41 => Self::Hash,
            42 => Self::Equals,
            43 => Self::Minus,
            44 => Self::OpenParenthesis,
            45 => Self::CloseParenthesis,
            46 => Self::OpenBracket,
            47 => Self::CloseBracket,
            _ => Self::UnknownNode,
        }
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RawLanguage {}

impl Language for RawLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::from_raw(raw)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<RawLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<RawLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<RawLanguage>;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RawSyntaxTree {
    green: GreenNode,
}

impl RawSyntaxTree {
    pub fn from_green(green: GreenNode) -> Self {
        Self { green }
    }

    pub fn root(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn text(&self) -> String {
        self.root().text().to_string()
    }

    pub fn debug_dump(&self) -> String {
        let mut out = String::new();
        dump_node(&self.root(), 0, &mut out);
        out
    }
}

impl std::fmt::Debug for RawSyntaxTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.debug_dump())
    }
}

pub trait AstNode: Sized {
    fn can_cast(kind: SyntaxKind) -> bool;
    fn cast(syntax: SyntaxNode) -> Option<Self>;
    fn syntax(&self) -> &SyntaxNode;
}

macro_rules! impl_ast_node {
    ($name:ident, $kind:pat) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            syntax: SyntaxNode,
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                matches!(kind, $kind)
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                Self::can_cast(syntax.kind()).then_some(Self { syntax })
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }
    };
}

impl_ast_node!(SourceFile, SyntaxKind::Root | SyntaxKind::CompilationUnit);
impl_ast_node!(ModuleDeclaration, SyntaxKind::ModuleDeclaration);

impl SourceFile {
    pub fn modules(&self) -> impl Iterator<Item = ModuleDeclaration> + '_ {
        self.syntax.children().filter_map(ModuleDeclaration::cast)
    }
}

impl ModuleDeclaration {
    pub fn name(&self) -> Option<SyntaxToken> {
        first_token(self.syntax(), SyntaxKind::Identifier)
    }
}

fn first_token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(child) => {
                if let Some(token) = first_token(&child, kind) {
                    return Some(token);
                }
            }
            NodeOrToken::Token(token) if token.kind() == kind => return Some(token),
            NodeOrToken::Token(_) => {}
        }
    }

    None
}

fn dump_node(node: &SyntaxNode, indent: usize, out: &mut String) {
    let range = node.text_range();
    out.push_str(&format!(
        "{:indent$}{:?} {}..{}\n",
        "",
        node.kind(),
        u32::from(range.start()),
        u32::from(range.end()),
        indent = indent
    ));

    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(child) => dump_node(&child, indent + 2, out),
            NodeOrToken::Token(token) => dump_token(&token, indent + 2, out),
        }
    }
}

fn dump_token(token: &SyntaxToken, indent: usize, out: &mut String) {
    let range = token.text_range();
    out.push_str(&format!(
        "{:indent$}{:?} {}..{} {:?}\n",
        "",
        token.kind(),
        u32::from(range.start()),
        u32::from(range.end()),
        token.text().to_string(),
        indent = indent
    ));
}

#[cfg(test)]
mod tests {
    use rowan::GreenNodeBuilder;

    use super::*;

    #[test]
    fn raw_tree_text_is_lossless() {
        let tree = module_tree();

        assert_eq!(tree.text(), "module top; endmodule");
    }

    #[test]
    fn typed_module_wrapper_finds_name() {
        let tree = module_tree();
        let file = SourceFile::cast(tree.root()).unwrap();
        let module = file.modules().next().unwrap();

        assert_eq!(module.name().unwrap().text(), "top");
    }

    fn module_tree() -> RawSyntaxTree {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Root.into());
        builder.start_node(SyntaxKind::ModuleDeclaration.into());
        builder.token(SyntaxKind::ModuleKeyword.into(), "module");
        builder.token(SyntaxKind::Whitespace.into(), " ");
        builder.token(SyntaxKind::Identifier.into(), "top");
        builder.token(SyntaxKind::Semicolon.into(), ";");
        builder.token(SyntaxKind::Whitespace.into(), " ");
        builder.token(SyntaxKind::EndModuleKeyword.into(), "endmodule");
        builder.finish_node();
        builder.finish_node();
        RawSyntaxTree::from_green(builder.finish())
    }
}
