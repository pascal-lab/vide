use super::{
    syntax_kind::SyntaxKind,
    syntax_node::{SyntaxNode, SyntaxTokenWithParent},
};
use crate::{source_buffer::SourceRange, token::TokenKind};

/// The kind of an untyped syntax element, either a node kind or a token kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntaxElementKind {
    Node(SyntaxKind),
    Token(TokenKind),
}

/// A child element in the concrete syntax tree.
/// Slang syntax children can be either nested syntax nodes or tokens. Token
/// elements carry their parent node because upstream token values do not store
/// a parent pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntaxElement<'a> {
    Node(SyntaxNode<'a>),
    Token(SyntaxTokenWithParent<'a>),
}

impl<'a> SyntaxElement<'a> {
    pub fn kind(self) -> SyntaxElementKind {
        match self {
            Self::Node(node) => SyntaxElementKind::Node(node.kind()),
            Self::Token(token) => SyntaxElementKind::Token(token.kind()),
        }
    }

    pub fn as_node(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::Node(node) => Some(node),
            Self::Token(_) => None,
        }
    }

    pub fn as_token(self) -> Option<SyntaxTokenWithParent<'a>> {
        match self {
            Self::Node(_) => None,
            Self::Token(token) => Some(token),
        }
    }

    pub fn parent(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::Node(node) => node.parent(),
            Self::Token(token) => Some(token.parent),
        }
    }

    pub fn range(self) -> Option<SourceRange> {
        match self {
            Self::Node(node) => node.range(),
            Self::Token(token) => token.range(),
        }
    }
}

impl<'a> From<SyntaxNode<'a>> for SyntaxElement<'a> {
    fn from(node: SyntaxNode<'a>) -> Self {
        Self::Node(node)
    }
}

impl<'a> From<SyntaxTokenWithParent<'a>> for SyntaxElement<'a> {
    fn from(token: SyntaxTokenWithParent<'a>) -> Self {
        Self::Token(token)
    }
}
