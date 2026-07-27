//! This file defines the untyped syntax tree nodes.
use std::{marker::PhantomData, ptr::NonNull};

use cxx::SharedPtr;

use super::{ffi, syntax_kind::SyntaxKind};
use crate::token::TokenKind;

#[derive(Clone)]
pub struct SyntaxTree {
    raw: SharedPtr<ffi::SyntaxTree>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntaxElement<'a> {
    Node(SyntaxNode<'a>),
    Token(SyntaxToken<'a>),
}

/// Like rust-alanyzer's `SyntaxNode` this is a untyped ast node.
/// You need to downcast it into `ASTNode` to get typed accessors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxNode<'a> {
    raw: NonNull<ffi::SyntaxNode>,
    _marker: PhantomData<&'a SyntaxTree>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxToken<'a> {
    raw: NonNull<ffi::SyntaxToken>,
    _marker: PhantomData<&'a SyntaxTree>,
}

#[derive(Clone, Debug)]
pub struct SyntaxChildrens<'a> {
    parent: SyntaxNode<'a>,
    index: usize,
    len: usize,
}

impl SyntaxTree {
    pub fn from_text(text: &str, name: &str, path: &str) -> Self {
        Self { raw: ffi::parse_syntax_tree(text, name, path) }
    }

    pub fn root(&self) -> Option<SyntaxNode<'_>> {
        SyntaxNode::from_raw(ffi::syntax_tree_root(self.raw.as_ref()?))
    }
}

impl<'a> SyntaxNode<'a> {
    fn from_raw(raw: *const ffi::SyntaxNode) -> Option<SyntaxNode<'a>> {
        NonNull::new(raw.cast_mut()).map(|raw| SyntaxNode { raw, _marker: PhantomData })
    }

    pub fn kind(self) -> SyntaxKind {
        SyntaxKind::from_raw(unsafe { ffi::syntax_node_kind(self.raw.as_ptr()) })
    }

    pub fn child_count(self) -> usize {
        unsafe { ffi::syntax_node_child_count(self.raw.as_ptr()) }
    }

    pub(crate) fn list_child_count(self) -> usize {
        unsafe { ffi::syntax_node_list_child_count(self.raw.as_ptr()) }
    }

    pub(crate) fn list_child_size(self, index: usize) -> Option<usize> {
        (index < self.list_child_count())
            .then(|| unsafe { ffi::syntax_node_list_child_size(self.raw.as_ptr(), index) })
    }

    pub fn child_node(self, index: usize) -> Option<SyntaxNode<'a>> {
        SyntaxNode::from_raw(unsafe { ffi::syntax_node_child_node(self.raw.as_ptr(), index) })
    }

    pub fn child_token(self, index: usize) -> Option<SyntaxToken<'a>> {
        SyntaxToken::from_raw(unsafe { ffi::syntax_node_child_token(self.raw.as_ptr(), index) })
    }

    pub fn child(self, index: usize) -> Option<SyntaxElement<'a>> {
        if index >= self.child_count() {
            return None;
        }
        self.child_node(index)
            .map(SyntaxElement::Node)
            .or_else(|| self.child_token(index).map(SyntaxElement::Token))
    }

    pub fn children(self) -> SyntaxChildrens<'a> {
        SyntaxChildrens { parent: self, index: 0, len: self.child_count() }
    }
}

impl<'a> SyntaxToken<'a> {
    fn from_raw(raw: *const ffi::SyntaxToken) -> Option<SyntaxToken<'a>> {
        NonNull::new(raw.cast_mut()).map(|raw| SyntaxToken { raw, _marker: PhantomData })
    }

    pub fn kind(self) -> TokenKind {
        TokenKind::from_raw(unsafe { ffi::syntax_token_kind(self.raw.as_ptr()) })
    }

    pub fn value_text(self) -> String {
        unsafe { ffi::syntax_token_value_text(self.raw.as_ptr()) }
    }
}

impl<'a> SyntaxElement<'a> {
    pub fn as_node(self) -> Option<SyntaxNode<'a>> {
        match self {
            Self::Node(node) => Some(node),
            Self::Token(_) => None,
        }
    }

    pub fn as_token(self) -> Option<SyntaxToken<'a>> {
        match self {
            Self::Node(_) => None,
            Self::Token(token) => Some(token),
        }
    }
}

impl<'a> Iterator for SyntaxChildrens<'a> {
    type Item = SyntaxElement<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.len {
            let index = self.index;
            self.index += 1;
            if let Some(child) = self.parent.child(index) {
                return Some(child);
            }
        }
        None
    }
}
