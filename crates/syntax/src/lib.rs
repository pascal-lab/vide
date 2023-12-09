pub mod ast;
pub mod syntax_kind;

use std::marker::PhantomData;

pub type SyntaxNode<'a> = tree_sitter::Node<'a>;

pub struct SyntaxChildren<'a> {
    cursor: Option<tree_sitter::TreeCursor<'a>>,
    ph: PhantomData<SyntaxNode<'a>>,
}

impl<'a> SyntaxChildren<'a> {
    pub fn new(parent: &'a SyntaxNode) -> Self {
        Self::new_from_node(parent.clone())
    }

    pub fn new_from_node(parent: SyntaxNode<'a>) -> Self {
        let mut cursor = parent.walk();
        cursor.reset(parent);
        let cursor = cursor.goto_first_child().then_some(cursor);
        SyntaxChildren { cursor, ph: PhantomData }
    }
}

impl<'a> Iterator for SyntaxChildren<'a> {
    type Item = SyntaxNode<'a>;
    fn next(&mut self) -> Option<SyntaxNode<'a>> {
        let cursor = self.cursor.as_mut()?;
        let cur_node = cursor.node();
        if !cursor.goto_next_sibling() {
            self.cursor = None;
        }
        Some(cur_node)
    }
}

pub struct SyntaxAncestors<'a> {
    node: Option<SyntaxNode<'a>>,
}

impl<'a> SyntaxAncestors<'a> {
    pub fn new(node: &'a SyntaxNode) -> Self {
        Self::new_from_node(node.clone())
    }

    pub fn new_from_node(node: SyntaxNode<'a>) -> Self {
        SyntaxAncestors { node: node.parent() }
    }
}

impl<'a> Iterator for SyntaxAncestors<'a> {
    type Item = SyntaxNode<'a>;
    fn next(&mut self) -> Option<SyntaxNode<'a>> {
        let node = self.node.take()?;
        self.node = node.parent();
        Some(node)
    }
}

pub struct SyntaxPreorder<'a> {
    root: SyntaxNode<'a>,
    cursor: tree_sitter::TreeCursor<'a>,
    finished: bool,
}

impl<'a> SyntaxPreorder<'a> {
    pub fn new(root: &'a SyntaxNode) -> Self {
        Self::new_from_node(root.clone())
    }

    pub fn new_from_node(root: SyntaxNode<'a>) -> Self {
        let mut cursor = root.walk();
        cursor.reset(root);
        SyntaxPreorder { root, cursor, finished: false }
    }
}

impl<'a> Iterator for SyntaxPreorder<'a> {
    type Item = SyntaxNode<'a>;

    fn next(&mut self) -> Option<SyntaxNode<'a>> {
        if self.finished {
            return None;
        }
        let cursor = &mut self.cursor;
        let cur_node = cursor.node();
        if cursor.goto_first_child() {
            return Some(cur_node);
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() || cursor.node() == self.root {
                self.finished = true;
                break;
            }
        }
        Some(cur_node)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxNodePtr {
    kind: &'static str,
    range: std::ops::Range<usize>,
}

impl SyntaxNodePtr {
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    pub fn from_node(node: &SyntaxNode) -> Self {
        let kind = node.kind();
        let range = node.byte_range();
        SyntaxNodePtr { kind, range }
    }

    pub fn to_node<'a>(&self, tree: &'a tree_sitter::Tree) -> Option<SyntaxNode<'a>> {
        let range = &self.range;
        let candidate = tree.root_node().descendant_for_byte_range(range.start, range.end)?;
        if candidate.kind() == self.kind {
            return Some(candidate);
        }
        let mut ancestors = SyntaxAncestors::new_from_node(candidate);

        while let Some(ancestor) = ancestors.next() {
            if ancestor.byte_range() != *range {
                break;
            } else if ancestor.kind() == self.kind {
                return Some(ancestor);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
