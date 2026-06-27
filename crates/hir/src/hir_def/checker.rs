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
    source_map::{FromSourceAst, IsNamedSrc, IsSrc, SourceAst, root_token_in},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CheckerDef {
    pub name: Option<Ident>,
}

pub type CheckerId = Idx<CheckerDef>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct CheckerSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

impl IsSrc for CheckerSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for CheckerSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> FromSourceAst<'a, ast::CheckerDeclaration<'a>> for CheckerSrc {
    fn from_source_ast(checker: SourceAst<ast::CheckerDeclaration<'a>>) -> Self {
        let checker = checker.into_inner();
        let syntax = checker.syntax();
        let name = checker
            .name()
            .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token));
        Self { node: AstNodeExt::to_ptr(&checker), name }
    }
}

pub fn lower_checker_decl(checker: ast::CheckerDeclaration<'_>) -> CheckerDef {
    CheckerDef { name: lower_ident_opt(checker.name()) }
}
