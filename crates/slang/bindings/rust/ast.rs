#![feature(associated_type_defaults)]
#![allow(unused)]
#![allow(clippy::enum_variant_names)]

use crate::{SyntaxChildren, SyntaxNode, SyntaxToken, syntax::SyntaxKind};

pub trait AstNode<'a>: Copy + Clone {
    fn can_cast(kind: SyntaxKind) -> bool
    where
        Self: Sized;

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self>
    where
        Self: Sized;

    fn syntax(&self) -> SyntaxNode<'a>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TokenList<'a> {
    syntax: SyntaxNode<'a>,
}

impl<'a> TokenList<'a> {
    pub fn children(&self) -> impl Iterator<Item = SyntaxToken<'a>> {
        SyntaxChildren::new(self.syntax).map(|elem| elem.as_token().unwrap())
    }
}

impl<'a> AstNode<'a> for TokenList<'a> {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TOKEN_LIST
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxList<'a, T: AstNode<'a>> {
    syntax: SyntaxNode<'a>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: AstNode<'a>> SyntaxList<'a, T> {
    pub fn children(&self) -> impl Iterator<Item = T> + 'a {
        SyntaxChildren::new(self.syntax).map(|elem| T::cast(elem.as_node().unwrap()).unwrap())
    }
}

impl<'a, T: AstNode<'a>> AstNode<'a> for SyntaxList<'a, T> {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SYNTAX_LIST
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax, _marker: std::marker::PhantomData })
    }

    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SeparatedList<'a, T: AstNode<'a>> {
    syntax: SyntaxNode<'a>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: AstNode<'a>> SeparatedList<'a, T> {
    pub fn children(&self) -> impl Iterator<Item = T> + 'a {
        SyntaxChildren::new(self.syntax)
            .step_by(2)
            .map(|elem| T::cast(elem.as_node().unwrap()).unwrap())
    }
}

impl<'a, T: AstNode<'a>> AstNode<'a> for SeparatedList<'a, T> {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SEPARATED_LIST
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax, _marker: std::marker::PhantomData })
    }

    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HybridNode<'a> {
    syntax: SyntaxNode<'a>,
}

impl<'a> AstNode<'a> for HybridNode<'a> {
    fn can_cast(_: SyntaxKind) -> bool {
        true
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Some(Self { syntax })
    }

    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }
}

include!(concat!(env!("OUT_DIR"), "/ast.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyntaxTree, TokenKind, syntax::SyntaxKind};

    fn get_test_tree() -> SyntaxTree {
        SyntaxTree::from_text("module AB(input a); wire x; endmodule;", "source", "")
    }

    fn get_multi_module_tree() -> SyntaxTree {
        SyntaxTree::from_text("module A; endmodule; module B; endmodule;", "source", "")
    }

    #[test]
    fn test_ast() {
        let tree = get_test_tree();
        let root = tree.root().unwrap();

        let unit = CompilationUnit::cast(root).unwrap();
        let module = match unit.members().children().next().unwrap() {
            Member::ModuleDeclaration(module) => module,
            _ => unreachable!("expected module declaration"),
        };
        let header = module.header();
        assert_eq!(header.module_keyword().unwrap().kind(), TokenKind::MODULE_KEYWORD);
        assert_eq!(header.name().unwrap().kind(), TokenKind::IDENTIFIER);
        let identifier = header.name().unwrap();
        assert_eq!(identifier.value_text().as_bytes(), b"AB");
    }

    #[test]
    fn multiple_module() {
        let tree = get_multi_module_tree();
        let root = tree.root().unwrap();

        let unit = CompilationUnit::cast(root).unwrap();
        let mut modules = unit.members().children().filter_map(Member::as_module_declaration);
        let module_a = modules.next().unwrap();
        let module_b = modules.next().unwrap();
        assert!(modules.next().is_none());

        let name_a = module_a.header();
        assert_eq!(name_a.name().unwrap().value_text().to_string(), "A");

        let name_b = module_b.header();
        assert_eq!(name_b.name().unwrap().value_text().to_string(), "B");
    }
}
