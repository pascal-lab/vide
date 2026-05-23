use crate::{SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTokenWithParent};

pub struct SyntaxCursor<'a> {
    elem: SyntaxElement<'a>,
    path: Vec<(SyntaxNode<'a>, usize)>,
}

impl<'a> SyntaxCursor<'a> {
    // Create a new cursor starting at the given node.
    pub fn new(root: SyntaxNode<'a>) -> SyntaxCursor<'a> {
        SyntaxCursor { elem: SyntaxElement::Node(root), path: Vec::with_capacity(16) }
    }

    pub fn to_elem(&self) -> SyntaxElement<'a> {
        self.elem
    }

    pub fn to_node(&self) -> Option<SyntaxNode<'a>> {
        self.elem.as_node()
    }

    pub fn to_tok_with_parent(&self) -> Option<SyntaxTokenWithParent<'a>> {
        self.elem.as_tok_with_parent()
    }

    pub fn to_token(&self) -> Option<SyntaxToken<'a>> {
        self.elem.as_token()
    }

    pub fn is_root(&self) -> bool {
        self.path.is_empty()
    }

    pub fn reset(&mut self, root: SyntaxNode<'a>) {
        self.elem = SyntaxElement::Node(root);
        self.path.clear();
    }

    pub fn reset_to_root(&mut self) {
        if self.path.is_empty() {
            return;
        }

        self.elem = self.path[0].0.into();
        self.path.clear();
    }

    // Returns the first child of the current node.
    //
    // If the current node has no children, returns None.
    pub fn goto_first_child(&mut self) -> bool {
        let Some((i, child)) = self.elem.children_with_idx().next() else {
            return false;
        };
        self.path.push((self.to_node().unwrap(), i));
        self.elem = child;
        true
    }

    // Move the cursor to the last child.
    //
    // Returns true if the cursor was moved.
    pub fn goto_last_child(&mut self) -> bool {
        let Some((i, child)) = self.elem.children_with_idx().last() else {
            return false;
        };
        self.path.push((self.to_node().unwrap(), i));
        self.elem = child;
        true
    }

    // Move the cursor to the parent of the current node.
    //
    // Returns true if the cursor was moved.
    pub fn goto_parent(&mut self) -> bool {
        let Some((parent, _)) = self.path.pop() else {
            return false;
        };
        self.elem = SyntaxElement::Node(parent);
        true
    }

    // Move the cursor to the next sibling.
    //
    // Returns true if the cursor was moved.
    pub fn goto_next_sibling(&mut self) -> bool {
        let Some((parent, idx)) = self.path.last_mut() else {
            return false;
        };

        let count = parent.child_count();
        while *idx < count - 1 {
            *idx += 1;
            if let Some(child) = parent.child(*idx) {
                self.elem = child;
                return true;
            }
        }
        false
    }

    // Move the cursor to the previous sibling.
    //
    // Returns true if the cursor was moved.
    pub fn goto_prev_sibling(&mut self) -> bool {
        let Some((parent, idx)) = self.path.last_mut() else {
            return false;
        };
        while *idx > 0 {
            *idx -= 1;
            if let Some(child) = parent.child(*idx) {
                self.elem = child;
                return true;
            }
        }
        false
    }

    // Returns the index of the current node in its parent's children.
    pub fn idx(&self) -> Option<usize> {
        self.path.last().map(|(_, idx)| *idx)
    }

    // Move the cursor to the first child that extends beyond the given position.
    //
    // Returns true if the cursor was moved.
    //
    // Example: given the following tree:
    //   root
    //     child1 (0..5)
    //     child2 (5..10)
    //
    // ```rust
    // let mut cursor = SyntaxCursor::new(tree);
    //
    // cursor.goto_first_child_after_pos(7);
    // assert_eq!(cursor.node.range(), Some(5..10));
    //
    // cursor.goto_first_child_after_pos(4);
    // assert_eq!(cursor.node.range(), Some(0..5));
    //
    // cursor.goto_first_child_after_pos(5);
    // assert_eq!(cursor.node.range(), Some(5..10));
    // ```
    pub fn goto_first_child_after_pos(&mut self, byte: usize) -> bool {
        let Some(node) = self.to_node() else {
            return false;
        };

        let tot = node.child_count();

        let mut left: i32 = 0;
        let mut right: i32 = (tot as i32) - 1;
        let mut result = None;

        'out: while left <= right {
            let mid = (left + right) / 2;
            let mut i = mid;
            let (child, range) = loop {
                if i > right {
                    right = mid - 1;
                    continue 'out;
                }
                let Some(child) = node.child(i as usize) else {
                    i += 1;
                    continue;
                };
                let Some(range) = child.range() else {
                    i += 1;
                    continue;
                };
                break (child, range);
            };
            if range.end() <= byte {
                left = mid + 1;
            } else {
                result = Some((child, i));
                right = mid - 1;
            }
        }

        if let Some((child, i)) = result {
            self.path.push((node, i as usize));
            self.elem = child;
            return true;
        }

        false
    }

    // Move the cursor to the last child that within the given position.
    //
    // Returns true if the cursor was moved.
    //
    // Example: given the following tree:
    //   root
    //     child1 (0..5)
    //     child2 (5..10)
    //
    // ```rust
    // let mut cursor = SyntaxCursor::new(tree);
    //
    // cursor.goto_last_child_before_pos(7);
    // assert_eq!(cursor.node.range(), Some(5..10));
    //
    // cursor.goto_last_child_before_pos(4);
    // assert_eq!(cursor.node.range(), Some(0..5));
    //
    // cursor.goto_last_child_before_pos(5);
    // assert_eq!(cursor.node.range(), Some(0..5));
    // ```
    pub fn goto_last_child_before_pos(&mut self, byte: usize) -> bool {
        let Some(node) = self.to_node() else {
            return false;
        };

        let tot = node.child_count();

        let mut left: i32 = 0;
        let mut right: i32 = (tot as i32) - 1;
        let mut result = None;

        'out: while left <= right {
            let mid = (left + right) / 2;
            let mut i = mid;
            let (child, range) = loop {
                if i < left {
                    left = mid + 1;
                    continue 'out;
                }
                let Some(child) = node.child(i as usize) else {
                    i -= 1;
                    continue;
                };
                let Some(range) = child.range() else {
                    i -= 1;
                    continue;
                };
                break (child, range);
            };
            if range.start() >= byte {
                right = mid - 1;
            } else {
                result = Some((child, i));
                left = mid + 1;
            }
        }

        if let Some((child, i)) = result {
            self.path.push((node, i as usize));
            self.elem = child;
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyntaxKind, SyntaxTree, TokenKind};

    fn get_test_tree() -> SyntaxTree {
        // A ident: 7..8
        // (: 8..9
        SyntaxTree::from_text("module A(input a); wire x; endmodule;", "source", "")
    }

    #[test]
    fn test_cursor() {
        let tree = get_test_tree();
        let mut cursor = SyntaxCursor::new(tree.root().unwrap());

        assert_eq!(cursor.idx(), None);
        assert_eq!(cursor.to_node().unwrap().kind(), SyntaxKind::COMPILATION_UNIT);

        assert!(cursor.goto_first_child());
        assert_eq!(cursor.idx(), Some(0));
        assert_eq!(cursor.to_node().unwrap().kind(), SyntaxKind::SYNTAX_LIST);

        assert!(cursor.goto_first_child());
        assert_eq!(cursor.to_node().unwrap().kind(), SyntaxKind::MODULE_DECLARATION);

        assert!(cursor.goto_first_child());
        assert!(cursor.goto_next_sibling());
        assert!(cursor.goto_first_child());
        assert_eq!(cursor.to_token().unwrap().kind(), TokenKind::MODULE_KEYWORD);

        assert!(cursor.goto_parent());
        assert!(cursor.goto_next_sibling());
        assert!(cursor.goto_next_sibling());
        assert_eq!(cursor.idx(), Some(3));
        assert_eq!(cursor.to_token().unwrap().kind(), TokenKind::END_MODULE_KEYWORD);

        assert!(cursor.goto_parent());
        assert!(cursor.goto_parent());
        assert_eq!(cursor.idx(), Some(0));
        assert_eq!(cursor.to_node().unwrap().kind(), SyntaxKind::SYNTAX_LIST);

        assert!(cursor.goto_last_child());
        assert_eq!(cursor.idx(), Some(1));
        assert_eq!(cursor.to_node().unwrap().kind(), SyntaxKind::EMPTY_MEMBER);

        cursor.reset_to_root();
        assert_eq!(cursor.idx(), None);
        assert_eq!(cursor.to_node().unwrap().kind(), SyntaxKind::COMPILATION_UNIT);
        assert!(cursor.goto_first_child_after_pos(8));
        assert!(cursor.goto_first_child_after_pos(8));
        assert!(cursor.goto_first_child_after_pos(8));
        assert!(cursor.goto_first_child_after_pos(8));
        assert!(cursor.goto_first_child_after_pos(8));
        assert_eq!(cursor.to_token().unwrap().kind(), TokenKind::OPEN_PARENTHESIS);

        cursor.reset_to_root();
        assert!(cursor.goto_last_child_before_pos(8));
        assert!(cursor.goto_last_child_before_pos(8));
        assert!(cursor.goto_last_child_before_pos(8));
        assert!(cursor.goto_last_child_before_pos(8));
        assert_eq!(cursor.to_token().unwrap().kind(), TokenKind::IDENTIFIER);
    }

    #[test]
    fn test_goto_by_byte() {
        let tree = get_test_tree();

        let mut cursor = SyntaxCursor::new(tree.root().unwrap());

        assert!(cursor.goto_first_child_after_pos(7));
        assert!(cursor.goto_first_child_after_pos(7));
        assert!(cursor.goto_first_child_after_pos(7));
        assert!(cursor.goto_first_child_after_pos(7));
        assert_eq!(cursor.to_token().unwrap().kind(), TokenKind::IDENTIFIER);
        assert!(cursor.goto_prev_sibling());
        assert_eq!(cursor.to_token().unwrap().kind(), TokenKind::MODULE_KEYWORD);

        cursor.reset_to_root();
        assert!(cursor.goto_last_child_before_pos(7));
        assert!(cursor.goto_last_child_before_pos(7));
        assert!(cursor.goto_last_child_before_pos(7));
        assert!(cursor.goto_last_child_before_pos(7));
        assert_eq!(cursor.to_token().unwrap().kind(), TokenKind::MODULE_KEYWORD);
        assert!(cursor.goto_next_sibling());
        assert_eq!(cursor.to_token().unwrap().kind(), TokenKind::IDENTIFIER);
    }
}
