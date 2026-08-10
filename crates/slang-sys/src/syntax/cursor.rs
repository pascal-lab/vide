use super::{
    element::SyntaxElement,
    syntax_node::{SyntaxNode, SyntaxToken, SyntaxTokenWithParent},
};

/// A movable cursor over syntax elements on a syntax tree.
#[derive(Clone, Debug)]
pub struct SyntaxCursor<'a> {
    elem: SyntaxElement<'a>,
    path: Vec<(SyntaxNode<'a>, usize)>,
}

impl<'a> SyntaxCursor<'a> {
    pub fn new(root: SyntaxNode<'a>) -> Self {
        Self { elem: SyntaxElement::Node(root), path: Vec::with_capacity(16) }
    }

    pub fn to_elem(&self) -> SyntaxElement<'a> {
        self.elem
    }

    pub fn to_node(&self) -> Option<SyntaxNode<'a>> {
        self.elem.as_node()
    }

    pub fn to_tok_with_parent(&self) -> Option<SyntaxTokenWithParent<'a>> {
        self.elem.as_token()
    }

    pub fn to_token(&self) -> Option<SyntaxToken<'a>> {
        self.to_tok_with_parent().map(|tok| tok.tok)
    }

    pub fn idx(&self) -> Option<usize> {
        self.path.last().map(|(_, index)| *index)
    }

    pub fn is_root(&self) -> bool {
        self.path.is_empty()
    }

    pub fn reset_to_root(&mut self) {
        if let Some((root, _)) = self.path.first().copied() {
            self.elem = SyntaxElement::Node(root);
            self.path.clear();
        }
    }

    pub fn goto_first_child(&mut self) -> bool {
        let Some(node) = self.to_node() else {
            return false;
        };
        let Some((index, child)) = node.children_with_idx().next() else {
            return false;
        };
        self.path.push((node, index));
        self.elem = child;
        true
    }

    pub fn goto_last_child(&mut self) -> bool {
        let Some(node) = self.to_node() else {
            return false;
        };
        let Some((index, child)) = node.children_with_idx().last() else {
            return false;
        };
        self.path.push((node, index));
        self.elem = child;
        true
    }

    pub fn goto_parent(&mut self) -> bool {
        let Some((parent, _)) = self.path.pop() else {
            return false;
        };
        self.elem = SyntaxElement::Node(parent);
        true
    }

    pub fn goto_next_sibling(&mut self) -> bool {
        let Some((parent, index)) = self.path.last_mut() else {
            return false;
        };
        while *index + 1 < parent.child_count() {
            *index += 1;
            if let Some(child) = parent.child(*index) {
                self.elem = child;
                return true;
            }
        }
        false
    }

    pub fn goto_prev_sibling(&mut self) -> bool {
        let Some((parent, index)) = self.path.last_mut() else {
            return false;
        };
        while *index > 0 {
            *index -= 1;
            if let Some(child) = parent.child(*index) {
                self.elem = child;
                return true;
            }
        }
        false
    }

    pub fn goto_first_child_after_pos(&mut self, byte: usize) -> bool {
        let Some(node) = self.to_node() else {
            return false;
        };
        for (index, child) in node.children_with_idx() {
            if child.range().is_some_and(|range| range.end() > byte) {
                self.path.push((node, index));
                self.elem = child;
                return true;
            }
        }
        false
    }

    pub fn goto_last_child_before_pos(&mut self, byte: usize) -> bool {
        let Some(node) = self.to_node() else {
            return false;
        };
        for (index, child) in node.children_with_idx().rev() {
            if child.range().is_some_and(|range| range.start() < byte) {
                self.path.push((node, index));
                self.elem = child;
                return true;
            }
        }
        false
    }
}
