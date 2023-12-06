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
        let mut cursor = parent.walk();
        cursor.reset(*parent);
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


#[cfg(test)]
mod tests;
