use crate::{SyntaxCursor, SyntaxElement, SyntaxNode};

pub struct SyntaxIdxChildren<'a> {
    parent: SyntaxNode<'a>,
    start_idx: usize,
    end_idx: usize,
}

impl<'a> SyntaxIdxChildren<'a> {
    pub fn new(parent: SyntaxNode<'a>) -> Self {
        SyntaxIdxChildren { parent, start_idx: 0, end_idx: parent.child_count() }
    }
}

impl<'a> Iterator for SyntaxIdxChildren<'a> {
    type Item = (usize, SyntaxElement<'a>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.start_idx < self.end_idx {
            let idx = self.start_idx;
            self.start_idx += 1;
            if let Some(child) = self.parent.child(idx) {
                return Some((idx, child));
            }
        }
        None
    }
}

impl<'a> DoubleEndedIterator for SyntaxIdxChildren<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.start_idx < self.end_idx {
            self.end_idx -= 1;
            if let Some(child) = self.parent.child(self.end_idx) {
                return Some((self.end_idx, child));
            }
        }
        None
    }
}

impl<'a> ExactSizeIterator for SyntaxIdxChildren<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.end_idx - self.start_idx
    }
}

pub struct SyntaxChildren<'a>(SyntaxIdxChildren<'a>);

impl<'a> SyntaxChildren<'a> {
    pub fn new(parent: SyntaxNode<'a>) -> Self {
        SyntaxChildren(SyntaxIdxChildren::new(parent))
    }
}

impl<'a> Iterator for SyntaxChildren<'a> {
    type Item = SyntaxElement<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, elem)| elem)
    }
}

impl<'a> DoubleEndedIterator for SyntaxChildren<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, elem)| elem)
    }
}

impl<'a> ExactSizeIterator for SyntaxChildren<'a> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// An iterator over the ancestors of a syntax node. The iterator returns the
/// node itself first and then its parent, grandparent, etc.
pub struct SyntaxAncestors<'a> {
    node: Option<SyntaxNode<'a>>,
}

impl<'a> SyntaxAncestors<'a> {
    pub fn start_from(node: SyntaxNode<'a>) -> Self {
        SyntaxAncestors { node: Some(node) }
    }
}

impl<'a> Iterator for SyntaxAncestors<'a> {
    type Item = SyntaxNode<'a>;

    fn next(&mut self) -> Option<SyntaxNode<'a>> {
        let res = self.node.take()?;
        self.node = res.parent();
        Some(res)
    }
}

pub enum WalkEvent<T> {
    Enter(T),
    Leave(T),
}

pub struct SyntaxNodePreorder<'a> {
    cursor: SyntaxCursor<'a>,
    leaving: bool,
}

impl<'a> SyntaxNodePreorder<'a> {
    pub fn new(root: SyntaxNode<'a>) -> Self {
        let cursor = SyntaxCursor::new(root);
        SyntaxNodePreorder { cursor, leaving: false }
    }

    // Skip the subtree including current node.
    pub fn skip_subtree(&mut self) {
        assert!(!self.leaving);
        self.leaving = true;
        self.cursor.goto_parent();
    }
}

impl<'a> Iterator for SyntaxNodePreorder<'a> {
    type Item = WalkEvent<SyntaxNode<'a>>;

    fn next(&mut self) -> Option<WalkEvent<SyntaxNode<'a>>> {
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

pub struct SyntaxElemPreorder<'a> {
    cursor: SyntaxCursor<'a>,
    leaving: bool,
}

impl<'a> SyntaxElemPreorder<'a> {
    pub fn new(root: SyntaxNode<'a>) -> Self {
        let cursor = SyntaxCursor::new(root);
        SyntaxElemPreorder { cursor, leaving: false }
    }

    // Skip the subtree rooted at the current node.
    pub fn skip_subtree(&mut self) {
        assert!(!self.leaving);
        self.leaving = true;
        self.cursor.goto_parent();
    }
}

impl<'a> Iterator for SyntaxElemPreorder<'a> {
    type Item = WalkEvent<SyntaxElement<'a>>;

    fn next(&mut self) -> Option<WalkEvent<SyntaxElement<'a>>> {
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

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use itertools::Itertools;

    use super::{SyntaxIdxChildren, SyntaxNodePreorder, WalkEvent};
    use crate::{SyntaxAncestors, SyntaxElemPreorder, SyntaxElementKind, SyntaxKind, SyntaxTree};

    fn get_test_tree() -> SyntaxTree {
        SyntaxTree::from_text("module A(input a); wire x; endmodule;", "source", "")
    }

    #[test]
    fn test_syntax_preorder() {
        let tree = get_test_tree();
        let root = tree.root().unwrap();

        let ans = SyntaxElemPreorder::new(root)
            .map(|event| match event {
                WalkEvent::Enter(elem) => format!("Enter({:?})", elem.kind()),
                WalkEvent::Leave(elem) => format!("Leave({:?})", elem.kind()),
            })
            .join("\n");
        let expected = expect![[r#"
            Enter(Node(CompilationUnit))
            Enter(Node(SyntaxList))
            Enter(Node(ModuleDeclaration))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Node(ModuleHeader))
            Enter(Token(ModuleKeyword))
            Leave(Token(ModuleKeyword))
            Enter(Token(Identifier))
            Leave(Token(Identifier))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Node(AnsiPortList))
            Enter(Token(OpenParenthesis))
            Leave(Token(OpenParenthesis))
            Enter(Node(SeparatedList))
            Enter(Node(ImplicitAnsiPort))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Node(VariablePortHeader))
            Enter(Token(InputKeyword))
            Leave(Token(InputKeyword))
            Enter(Node(ImplicitType))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Token(Placeholder))
            Leave(Token(Placeholder))
            Leave(Node(ImplicitType))
            Leave(Node(VariablePortHeader))
            Enter(Node(Declarator))
            Enter(Token(Identifier))
            Leave(Token(Identifier))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Leave(Node(Declarator))
            Leave(Node(ImplicitAnsiPort))
            Leave(Node(SeparatedList))
            Enter(Token(CloseParenthesis))
            Leave(Token(CloseParenthesis))
            Leave(Node(AnsiPortList))
            Enter(Token(Semicolon))
            Leave(Token(Semicolon))
            Leave(Node(ModuleHeader))
            Enter(Node(SyntaxList))
            Enter(Node(NetDeclaration))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Token(WireKeyword))
            Leave(Token(WireKeyword))
            Enter(Node(ImplicitType))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Token(Placeholder))
            Leave(Token(Placeholder))
            Leave(Node(ImplicitType))
            Enter(Node(SeparatedList))
            Enter(Node(Declarator))
            Enter(Token(Identifier))
            Leave(Token(Identifier))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Leave(Node(Declarator))
            Leave(Node(SeparatedList))
            Enter(Token(Semicolon))
            Leave(Token(Semicolon))
            Leave(Node(NetDeclaration))
            Leave(Node(SyntaxList))
            Enter(Token(EndModuleKeyword))
            Leave(Token(EndModuleKeyword))
            Leave(Node(ModuleDeclaration))
            Enter(Node(EmptyMember))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Node(TokenList))
            Leave(Node(TokenList))
            Enter(Token(Semicolon))
            Leave(Token(Semicolon))
            Leave(Node(EmptyMember))
            Leave(Node(SyntaxList))
            Enter(Token(EndOfFile))
            Leave(Token(EndOfFile))"#]];
        expected.assert_eq(&ans);
    }

    #[test]
    fn test_skip_subtree() {
        let tree = get_test_tree();
        let root = tree.root().unwrap();

        let mut iter = SyntaxElemPreorder::new(root);
        let mut ans = Vec::new();
        while let Some(event) = iter.next() {
            match event {
                WalkEvent::Enter(elem) => {
                    ans.push(format!("Enter({:?})", elem.kind()));
                    if elem.kind() == SyntaxElementKind::Node(SyntaxKind::NET_DECLARATION) {
                        iter.skip_subtree();
                    }
                }
                WalkEvent::Leave(elem) => {
                    ans.push(format!("Leave({:?})", elem.kind()));
                }
            }
        }

        let expected = expect![[r#"
            Enter(Node(CompilationUnit))
            Enter(Node(SyntaxList))
            Enter(Node(ModuleDeclaration))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Node(ModuleHeader))
            Enter(Token(ModuleKeyword))
            Leave(Token(ModuleKeyword))
            Enter(Token(Identifier))
            Leave(Token(Identifier))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Node(AnsiPortList))
            Enter(Token(OpenParenthesis))
            Leave(Token(OpenParenthesis))
            Enter(Node(SeparatedList))
            Enter(Node(ImplicitAnsiPort))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Node(VariablePortHeader))
            Enter(Token(InputKeyword))
            Leave(Token(InputKeyword))
            Enter(Node(ImplicitType))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Token(Placeholder))
            Leave(Token(Placeholder))
            Leave(Node(ImplicitType))
            Leave(Node(VariablePortHeader))
            Enter(Node(Declarator))
            Enter(Token(Identifier))
            Leave(Token(Identifier))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Leave(Node(Declarator))
            Leave(Node(ImplicitAnsiPort))
            Leave(Node(SeparatedList))
            Enter(Token(CloseParenthesis))
            Leave(Token(CloseParenthesis))
            Leave(Node(AnsiPortList))
            Enter(Token(Semicolon))
            Leave(Token(Semicolon))
            Leave(Node(ModuleHeader))
            Enter(Node(SyntaxList))
            Enter(Node(NetDeclaration))
            Leave(Node(NetDeclaration))
            Leave(Node(SyntaxList))
            Enter(Token(EndModuleKeyword))
            Leave(Token(EndModuleKeyword))
            Leave(Node(ModuleDeclaration))
            Enter(Node(EmptyMember))
            Enter(Node(SyntaxList))
            Leave(Node(SyntaxList))
            Enter(Node(TokenList))
            Leave(Node(TokenList))
            Enter(Token(Semicolon))
            Leave(Token(Semicolon))
            Leave(Node(EmptyMember))
            Leave(Node(SyntaxList))
            Enter(Token(EndOfFile))
            Leave(Token(EndOfFile))"#]];
        expected.assert_eq(&ans.join("\n"));
    }

    #[test]
    fn test_skip_node_subtree() {
        let tree = get_test_tree();
        let root = tree.root().unwrap();

        let mut iter = SyntaxNodePreorder::new(root);
        let mut ans = Vec::new();
        while let Some(event) = iter.next() {
            match event {
                WalkEvent::Enter(elem) => {
                    ans.push(format!("Enter({:?})", elem.kind()));
                    if elem.kind() == SyntaxKind::NET_DECLARATION {
                        iter.skip_subtree();
                    }
                }
                WalkEvent::Leave(elem) => {
                    ans.push(format!("Leave({:?})", elem.kind()));
                }
            }
        }

        let expected = expect![[r#"
            Enter(CompilationUnit)
            Enter(SyntaxList)
            Enter(ModuleDeclaration)
            Enter(SyntaxList)
            Leave(SyntaxList)
            Enter(ModuleHeader)
            Enter(SyntaxList)
            Leave(SyntaxList)
            Enter(AnsiPortList)
            Enter(SeparatedList)
            Enter(ImplicitAnsiPort)
            Enter(SyntaxList)
            Leave(SyntaxList)
            Enter(VariablePortHeader)
            Enter(ImplicitType)
            Enter(SyntaxList)
            Leave(SyntaxList)
            Leave(ImplicitType)
            Leave(VariablePortHeader)
            Enter(Declarator)
            Enter(SyntaxList)
            Leave(SyntaxList)
            Leave(Declarator)
            Leave(ImplicitAnsiPort)
            Leave(SeparatedList)
            Leave(AnsiPortList)
            Leave(ModuleHeader)
            Enter(SyntaxList)
            Enter(NetDeclaration)
            Leave(NetDeclaration)
            Leave(SyntaxList)
            Leave(ModuleDeclaration)
            Enter(EmptyMember)
            Enter(SyntaxList)
            Leave(SyntaxList)
            Enter(TokenList)
            Leave(TokenList)
            Leave(EmptyMember)
            Leave(SyntaxList)"#]];
        expected.assert_eq(&ans.join("\n"));
    }

    #[test]
    fn test_syntax_children() {
        let tree = get_test_tree();
        let root = tree.root().unwrap();
        let node = root.child_node(0).unwrap();
        let node = node.child_node(0).unwrap();
        let node = node.child_node(1).unwrap();

        {
            let ans = SyntaxIdxChildren::new(node)
                .map(|(idx, elem)| format!("{idx}: {:?}", elem.kind()))
                .join("\n");
            let expected = expect![[r#"
                0: Token(ModuleKeyword)
                2: Token(Identifier)
                3: Node(SyntaxList)
                5: Node(AnsiPortList)
                6: Token(Semicolon)"#]];
            expected.assert_eq(&ans);
        }

        {
            let ans = SyntaxIdxChildren::new(node)
                .rev()
                .map(|(idx, elem)| format!("{idx}: {:?}", elem.kind()))
                .join("\n");
            let expected = expect![[r#"
                6: Token(Semicolon)
                5: Node(AnsiPortList)
                3: Node(SyntaxList)
                2: Token(Identifier)
                0: Token(ModuleKeyword)"#]];
            expected.assert_eq(&ans);
        }
    }

    #[test]
    fn test_syntax_ancestor() {
        let tree = get_test_tree();
        let root = tree.root().unwrap();
        let node = root.child_node(0).unwrap();
        let node = node.child_node(0).unwrap();
        let node = node.child_node(1).unwrap();

        let ans =
            SyntaxAncestors::start_from(node).map(|elem| format!("{:?}", elem.kind())).join("\n");
        let expected = expect![[r#"
            ModuleHeader
            ModuleDeclaration
            CompilationUnit"#]];
        expected.assert_eq(&ans);
    }
}
