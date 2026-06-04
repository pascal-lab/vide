use la_arena::Idx;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
    slang_ext::AstNodeExt,
};
use utils::text_edit::TextRange;

use crate::{
    hir_def::{Ident, lower_ident_opt},
    source_map::{FromSourceAst, IsNamedSrc, IsSrc, SourceAst, ToAstNode, root_token_in},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Modport {
    pub name: Option<Ident>,
}

pub type ModportId = Idx<Modport>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ModportSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

impl IsSrc for ModportSrc {
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for ModportSrc {
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> ToAstNode<'a, ast::ModportItem<'a>> for ModportSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::ModportItem<'a>> {
        ast::ModportItem::cast(self.node.to_node(tree)?)
    }
}

impl From<ast::ModportItem<'_>> for ModportSrc {
    fn from(node: ast::ModportItem<'_>) -> Self {
        let syntax = node.syntax();
        let name = node.name().map(|name| SyntaxTokenPtr::from_token_in(syntax, name));
        ModportSrc { node: AstNodeExt::to_ptr(&node), name }
    }
}

impl<'a> FromSourceAst<'a, ast::ModportItem<'a>> for ModportSrc {
    fn from_source_ast(node: SourceAst<ast::ModportItem<'a>>) -> Self {
        let node = node.into_inner();
        let syntax = node.syntax();
        let name = node
            .name()
            .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token));
        ModportSrc { node: AstNodeExt::to_ptr(&node), name }
    }
}

pub(crate) fn lower_modport_item(item: ast::ModportItem) -> Modport {
    Modport { name: lower_ident_opt(item.name()) }
}
