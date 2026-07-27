use super::{cursor::SyntaxCursor, element::SyntaxElement, syntax_node::SyntaxNode};

/// Event emitted by preorder syntax tree walks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WalkEvent<T> {
    Enter(T),
    Leave(T),
}

/// Preorder traversal over syntax nodes.
#[derive(Clone, Debug)]
pub struct SyntaxNodePreorder<'a> {
    cursor: SyntaxCursor<'a>,
    leaving: bool,
}

impl<'a> SyntaxNodePreorder<'a> {
    pub fn new(root: SyntaxNode<'a>) -> Self {
        Self { cursor: SyntaxCursor::new(root), leaving: false }
    }
}

impl<'a> Iterator for SyntaxNodePreorder<'a> {
    type Item = WalkEvent<SyntaxNode<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.leaving && self.cursor.is_root() {
            return None;
        }

        let event = if self.leaving {
            WalkEvent::Leave(self.cursor.to_node().unwrap())
        } else {
            WalkEvent::Enter(self.cursor.to_node().unwrap())
        };

        if self.leaving {
            loop {
                if !self.cursor.goto_next_sibling() {
                    self.cursor.goto_parent();
                    break;
                } else if self.cursor.to_node().is_some() {
                    self.leaving = false;
                    break;
                }
            }
        } else if self.cursor.goto_first_child() {
            loop {
                if self.cursor.to_node().is_some() {
                    break;
                } else if !self.cursor.goto_next_sibling() {
                    self.leaving = true;
                    self.cursor.goto_parent();
                    break;
                }
            }
        } else {
            self.leaving = true;
        }

        Some(event)
    }
}

#[derive(Clone, Debug)]
/// Preorder traversal over both syntax nodes and tokens.
pub struct SyntaxElemPreorder<'a> {
    cursor: SyntaxCursor<'a>,
    leaving: bool,
}

impl<'a> SyntaxElemPreorder<'a> {
    pub fn new(root: SyntaxNode<'a>) -> Self {
        Self { cursor: SyntaxCursor::new(root), leaving: false }
    }
}

impl<'a> Iterator for SyntaxElemPreorder<'a> {
    type Item = WalkEvent<SyntaxElement<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.leaving && self.cursor.is_root() {
            return None;
        }

        let event = if self.leaving {
            WalkEvent::Leave(self.cursor.to_elem())
        } else {
            WalkEvent::Enter(self.cursor.to_elem())
        };

        if self.leaving {
            if self.cursor.goto_next_sibling() {
                self.leaving = false;
            } else {
                self.cursor.goto_parent();
            }
        } else if !self.cursor.goto_first_child() {
            self.leaving = true;
        }

        Some(event)
    }
}
