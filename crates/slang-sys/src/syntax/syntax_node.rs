//! Untyped syntax tree node and token views.
use std::{hash, marker::PhantomData, ptr::NonNull};

use super::{
    cursor::SyntaxCursor,
    element::SyntaxElement,
    ffi,
    iter::{ChildrenIter, SyntaxChildren, SyntaxIdxChildren},
    syntax_kind::SyntaxKind,
    tree::SyntaxTree,
    trivia::{SyntaxTrivia, SyntaxTriviaIter, SyntaxTriviaLoc},
    walk::{SyntaxElemPreorder, SyntaxNodePreorder},
};
use crate::{source_buffer::SourceRange, token::TokenKind};

/// An untyped Slang syntax node.
/// Downcast this into an `ast::AstNode` to use generated typed accessors.
#[derive(Clone, Copy, Debug)]
pub struct SyntaxNode<'a> {
    pub(crate) raw: NonNull<ffi::SyntaxNode>,
    pub(crate) _marker: PhantomData<&'a SyntaxTree>,
}

/// An untyped Slang syntax token.
#[derive(Clone, Copy, Debug)]
pub struct SyntaxToken<'a> {
    pub(crate) raw: NonNull<ffi::SyntaxToken>,
    pub(crate) _marker: PhantomData<&'a SyntaxTree>,
}

/// A syntax token paired with its parent node.
/// Slang token values do not carry a parent pointer, so traversal code keeps
/// the parent next to the token when it needs context-sensitive operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxTokenWithParent<'a> {
    pub parent: SyntaxNode<'a>,
    pub tok: SyntaxToken<'a>,
}

impl<'a> SyntaxNode<'a> {
    pub(crate) fn from_nullable_raw(raw: *const ffi::SyntaxNode) -> Option<SyntaxNode<'a>> {
        NonNull::new(raw.cast_mut()).map(|raw| SyntaxNode { raw, _marker: PhantomData })
    }

    pub fn kind(self) -> SyntaxKind {
        SyntaxKind::from_raw(unsafe { ffi::syntax_node_kind(self.raw.as_ptr()) })
    }

    pub fn range(self) -> Option<SourceRange> {
        let valid = unsafe { ffi::syntax_node_range_valid(self.raw.as_ptr()) };
        if !valid {
            return None;
        }

        Some(SourceRange::from_parts(
            unsafe { ffi::syntax_node_range_start_buffer_id(self.raw.as_ptr()) },
            unsafe { ffi::syntax_node_range_start_offset(self.raw.as_ptr()) },
            unsafe { ffi::syntax_node_range_end_buffer_id(self.raw.as_ptr()) },
            unsafe { ffi::syntax_node_range_end_offset(self.raw.as_ptr()) },
        ))
    }

    pub fn range_with_context(self, context: SyntaxNode<'a>) -> Option<SourceRange> {
        let valid = unsafe {
            ffi::syntax_node_range_with_context_valid(self.raw.as_ptr(), context.raw.as_ptr())
        };
        if !valid {
            return None;
        }

        Some(SourceRange::from_parts(
            unsafe {
                ffi::syntax_node_range_with_context_start_buffer_id(
                    self.raw.as_ptr(),
                    context.raw.as_ptr(),
                )
            },
            unsafe {
                ffi::syntax_node_range_with_context_start_offset(
                    self.raw.as_ptr(),
                    context.raw.as_ptr(),
                )
            },
            unsafe {
                ffi::syntax_node_range_with_context_end_buffer_id(
                    self.raw.as_ptr(),
                    context.raw.as_ptr(),
                )
            },
            unsafe {
                ffi::syntax_node_range_with_context_end_offset(
                    self.raw.as_ptr(),
                    context.raw.as_ptr(),
                )
            },
        ))
    }

    pub fn parent(self) -> Option<SyntaxNode<'a>> {
        SyntaxNode::from_nullable_raw(unsafe { ffi::syntax_node_parent(self.raw.as_ptr()) })
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
        SyntaxNode::from_nullable_raw(unsafe {
            ffi::syntax_node_child_node(self.raw.as_ptr(), index)
        })
    }

    pub fn child_token(self, index: usize) -> Option<SyntaxToken<'a>> {
        SyntaxToken::from_nullable_raw(unsafe {
            ffi::syntax_node_child_token(self.raw.as_ptr(), index)
        })
        .filter(|tok| tok.kind() != TokenKind::UNKNOWN)
    }

    pub fn child(self, index: usize) -> Option<SyntaxElement<'a>> {
        if index >= self.child_count() {
            return None;
        }
        self.child_node(index).map(SyntaxElement::Node).or_else(|| {
            self.child_token(index)
                .map(|tok| SyntaxElement::Token(SyntaxTokenWithParent { parent: self, tok }))
        })
    }

    pub fn children_with_idx(self) -> SyntaxIdxChildren<'a> {
        SyntaxIdxChildren { parent: self, start_idx: 0, end_idx: self.child_count() }
    }

    pub fn children(self) -> SyntaxChildren<'a> {
        SyntaxChildren(self.children_with_idx())
    }

    pub fn walk(self) -> SyntaxCursor<'a> {
        SyntaxCursor::new(self)
    }

    pub fn first_token(self) -> Option<SyntaxTokenWithParent<'a>> {
        let mut cursor = self.walk();
        while cursor.to_tok_with_parent().is_none() {
            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return None;
                }
            }
        }
        cursor.to_tok_with_parent()
    }

    pub fn node_preorder(self) -> SyntaxNodePreorder<'a> {
        SyntaxNodePreorder::new(self)
    }

    pub fn elem_preorder(self) -> SyntaxElemPreorder<'a> {
        SyntaxElemPreorder::new(self)
    }
}

impl PartialEq for SyntaxNode<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for SyntaxNode<'_> {}

impl hash::Hash for SyntaxNode<'_> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<'a> SyntaxToken<'a> {
    fn from_nullable_raw(raw: *const ffi::SyntaxToken) -> Option<SyntaxToken<'a>> {
        NonNull::new(raw.cast_mut()).map(|raw| SyntaxToken { raw, _marker: PhantomData })
    }

    pub fn kind(self) -> TokenKind {
        TokenKind::from_raw(unsafe { ffi::syntax_token_kind(self.raw.as_ptr()) })
    }

    pub fn range(self) -> Option<SourceRange> {
        let valid = unsafe { ffi::syntax_token_range_valid(self.raw.as_ptr()) };
        if !valid {
            return None;
        }

        Some(SourceRange::from_parts(
            unsafe { ffi::syntax_token_range_start_buffer_id(self.raw.as_ptr()) },
            unsafe { ffi::syntax_token_range_start_offset(self.raw.as_ptr()) },
            unsafe { ffi::syntax_token_range_end_buffer_id(self.raw.as_ptr()) },
            unsafe { ffi::syntax_token_range_end_offset(self.raw.as_ptr()) },
        ))
    }

    pub fn range_with_context(self, _context: SyntaxNode<'a>) -> Option<SourceRange> {
        let valid = unsafe {
            ffi::syntax_token_range_with_context_valid(self.raw.as_ptr(), _context.raw.as_ptr())
        };
        if !valid {
            return None;
        }

        Some(SourceRange::from_parts(
            unsafe {
                ffi::syntax_token_range_with_context_start_buffer_id(
                    self.raw.as_ptr(),
                    _context.raw.as_ptr(),
                )
            },
            unsafe {
                ffi::syntax_token_range_with_context_start_offset(
                    self.raw.as_ptr(),
                    _context.raw.as_ptr(),
                )
            },
            unsafe {
                ffi::syntax_token_range_with_context_end_buffer_id(
                    self.raw.as_ptr(),
                    _context.raw.as_ptr(),
                )
            },
            unsafe {
                ffi::syntax_token_range_with_context_end_offset(
                    self.raw.as_ptr(),
                    _context.raw.as_ptr(),
                )
            },
        ))
    }

    pub fn value_text(self) -> String {
        unsafe { ffi::syntax_token_value_text(self.raw.as_ptr()) }
    }

    pub fn trivia_count(self) -> usize {
        unsafe { ffi::syntax_token_trivia_count(self.raw.as_ptr()) }
    }

    pub fn trivia_at(self, idx: usize) -> Option<SyntaxTrivia<'a>> {
        if idx >= self.trivia_count() {
            return None;
        }

        Some(SyntaxTrivia::from_raw(unsafe { ffi::syntax_token_trivia(self.raw.as_ptr(), idx) }))
    }

    pub fn trivias(self) -> SyntaxTriviaIter<'a> {
        SyntaxTriviaIter::new(self)
    }

    pub fn trivias_with_loc(
        self,
    ) -> impl ChildrenIter<(SyntaxTriviaLoc, SyntaxTrivia<'a>)> + use<'a> {
        let mut locs = Vec::with_capacity(self.trivia_count());

        if let Some(range) = self.range() {
            let mut cursor_buffer_id = range.start_buffer_id();
            let mut cursor_offset = range.start();

            for trivia in self.trivias().rev() {
                let len = trivia.get_raw_text().len();

                let loc = if let Some(location) = trivia.explicit_location() {
                    let start = location.offset();
                    SyntaxTriviaLoc { buffer_id: location.buffer_id(), start, end: start + len }
                } else {
                    let end = cursor_offset;
                    let start = end.saturating_sub(len);
                    SyntaxTriviaLoc { buffer_id: cursor_buffer_id, start, end }
                };

                cursor_buffer_id = loc.buffer_id;
                cursor_offset = loc.start;
                locs.push((loc, trivia));
            }
        }

        locs.into_iter().rev()
    }
}

impl PartialEq for SyntaxToken<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for SyntaxToken<'_> {}

impl hash::Hash for SyntaxToken<'_> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<'a> SyntaxTokenWithParent<'a> {
    pub fn kind(self) -> TokenKind {
        self.tok.kind()
    }

    pub fn range(self) -> Option<SourceRange> {
        self.tok.range_with_context(self.parent)
    }

    pub fn trivias(self) -> SyntaxTriviaIter<'a> {
        self.tok.trivias()
    }

    pub fn trivias_with_loc(
        self,
    ) -> impl ChildrenIter<(SyntaxTriviaLoc, SyntaxTrivia<'a>)> + use<'a> {
        self.tok.trivias_with_loc()
    }
}
