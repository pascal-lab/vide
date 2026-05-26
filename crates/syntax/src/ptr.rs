use slang::{
    SyntaxElement, SyntaxElementKind, SyntaxKind, SyntaxNode, SyntaxToken, SyntaxTokenWithParent,
    SyntaxTree, TokenKind,
};
use utils::line_index::TextRange;

use crate::{SyntaxNodeExt, has_text_range::HasTextRange};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxNodePtr {
    kind: SyntaxKind,
    range: TextRange,
}

impl SyntaxNodePtr {
    #[inline]
    pub fn from_node(node: SyntaxNode) -> SyntaxNodePtr {
        SyntaxNodePtr { kind: node.kind(), range: node.text_range().unwrap() }
    }

    #[inline]
    pub fn to_node<'a>(&self, tree: &'a SyntaxTree) -> Option<SyntaxNode<'a>> {
        let root_node = tree.root()?;
        root_node.elem_at_exact_range(self.range)?.as_node()
    }

    #[inline]
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    #[inline]
    pub fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxTokenPtr {
    kind: TokenKind,
    range: TextRange,
}

impl SyntaxTokenPtr {
    pub fn from_token(token: SyntaxTokenWithParent) -> SyntaxTokenPtr {
        SyntaxTokenPtr { kind: token.kind(), range: token.text_range().unwrap() }
    }

    pub fn from_token_in(context: SyntaxNode, token: SyntaxToken) -> SyntaxTokenPtr {
        SyntaxTokenPtr::from_token(SyntaxTokenWithParent { parent: context, tok: token })
    }

    pub fn to_token<'a>(&self, tree: &'a SyntaxTree) -> Option<SyntaxTokenWithParent<'a>> {
        tree.root()?.token_at_offset(self.range.start()).find(|token| {
            token.kind() == self.kind && token.text_range().is_some_and(|range| range == self.range)
        })
    }

    pub fn kind(&self) -> TokenKind {
        self.kind
    }

    pub fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SyntaxElementPtr {
    Node(SyntaxNodePtr),
    Token { parent: SyntaxNodePtr, tok: SyntaxTokenPtr },
}

impl SyntaxElementPtr {
    pub fn from_element(element: SyntaxElement) -> SyntaxElementPtr {
        match element {
            SyntaxElement::Node(node) => SyntaxElementPtr::Node(SyntaxNodePtr::from_node(node)),
            SyntaxElement::Token(tok_with_parent @ SyntaxTokenWithParent { parent, .. }) => {
                SyntaxElementPtr::Token {
                    parent: SyntaxNodePtr::from_node(parent),
                    tok: SyntaxTokenPtr::from_token(tok_with_parent),
                }
            }
        }
    }

    pub fn to_elem<'a>(&self, tree: &'a SyntaxTree) -> Option<SyntaxElement<'a>> {
        match self {
            SyntaxElementPtr::Node(node) => node.to_node(tree).map(SyntaxElement::from_node),
            SyntaxElementPtr::Token { tok, .. } => {
                Some(SyntaxElement::from_token(tok.to_token(tree)?))
            }
        }
    }

    pub fn kind(&self) -> SyntaxElementKind {
        match self {
            SyntaxElementPtr::Node(SyntaxNodePtr { kind, .. }) => SyntaxElementKind::Node(*kind),
            SyntaxElementPtr::Token { tok, .. } => SyntaxElementKind::Token(tok.kind),
        }
    }
}

#[cfg(test)]
mod tests {
    use slang::ast::{self, AstNode};

    use crate::has_text_range::HasTextRange;

    #[test]
    fn no_include_expansion_parse_does_not_expand_cwd_includes() {
        let include_rel = "target/vide_pathless_include_test_defs.svh";
        std::fs::create_dir_all("target").expect("target directory");
        std::fs::write(include_rel, "typedef logic cwd_include_t;\n").expect("include fixture");

        let text = format!("`include \"{include_rel}\"\nmodule top;\nendmodule\n");
        let tree = slang::SyntaxTree::from_text_with_options(
            &text,
            "",
            "",
            &slang::SyntaxTreeOptions::without_include_expansion(),
        );
        let root = tree.root().expect("root syntax node");
        let unit = ast::CompilationUnit::cast(root).expect("compilation unit");
        let mut saw_root_module = false;

        for member in unit.members().children() {
            match member {
                ast::Member::TypedefDeclaration(_) => {
                    panic!("parse with include expansion disabled must not read includes");
                }
                ast::Member::ModuleDeclaration(module) => {
                    saw_root_module = true;
                    assert!(module.syntax().text_range().is_some());
                }
                _ => {}
            }
        }

        assert!(saw_root_module);
    }
}
