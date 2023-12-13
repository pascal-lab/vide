pub mod ast;
pub mod syntax_kind;

pub type SyntaxNode<'a> = tree_sitter::Node<'a>;

pub struct SyntaxChildren<'a> {
    cursor: Option<tree_sitter::TreeCursor<'a>>,
}

impl<'a> SyntaxChildren<'a> {
    pub fn new(parent: &'a SyntaxNode) -> Self {
        Self::new_from_node(*parent)
    }

    pub fn new_from_node(parent: SyntaxNode<'a>) -> Self {
        let mut cursor = parent.walk();
        cursor.reset(parent);
        let cursor = cursor.goto_first_child().then_some(cursor);
        SyntaxChildren { cursor }
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
        Self::new_from_node(*node)
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
        Self::new_from_node(*root)
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

        let cur_node = self.cursor.node();

        let cursor = &mut self.cursor;
        if cursor.goto_first_child() {
            return Some(cur_node);
        }

        while !cursor.goto_next_sibling() {
            let has_parent = cursor.goto_parent();
            if !has_parent || cursor.node() == self.root {
                self.finished = true;
                break;
            }
        }
        Some(cur_node)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxNodePtr {
    kind_id: syntax_kind::SyntaxKindId,
    range: std::ops::Range<usize>,
}

impl SyntaxNodePtr {
    pub fn kind_id(&self) -> syntax_kind::SyntaxKindId {
        self.kind_id
    }

    pub fn from_node(node: &SyntaxNode) -> Self {
        let kind_id = node.kind_id();
        let range = node.byte_range();
        SyntaxNodePtr { kind_id, range }
    }

    pub fn to_node<'a>(&self, tree: &'a tree_sitter::Tree) -> Option<SyntaxNode<'a>> {
        let range = &self.range;
        let candidate = tree.root_node().descendant_for_byte_range(range.start, range.end)?;
        if candidate.kind_id() == self.kind_id {
            return Some(candidate);
        }

        for ancestor in SyntaxAncestors::new_from_node(candidate) {
            if ancestor.byte_range() != *range {
                break;
            } else if ancestor.kind_id() == self.kind_id {
                return Some(ancestor);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
