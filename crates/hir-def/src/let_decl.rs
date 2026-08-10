use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
};

use crate::{
    Ident, alloc_with_source_entry,
    assertion::AssertionPort,
    expr::ExprId,
    lower::{BodyStore, LoweringCtx, LoweringStore},
    lower_ident_opt,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LetDecl {
    pub name: Ident,
    pub ports: SmallVec<[AssertionPort; 4]>,
    pub expr: ExprId,
}

pub type LetDeclId = Idx<LetDecl>;

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_let_decl(&mut self, declaration: ast::LetDeclaration) -> Option<LetDeclId> {
        if declaration.let_().map(|token| token.kind()) != Some(TokenKind::LET_KEYWORD) {
            self.report_invalid(declaration.syntax(), "let declaration is missing its let keyword");
            return None;
        }
        let Some(name) = lower_ident_opt(declaration.identifier()) else {
            self.report_invalid(declaration.syntax(), "let declaration is missing its name");
            return None;
        };
        if declaration.equals().map(|token| token.kind()) != Some(TokenKind::EQUALS) {
            self.report_invalid(declaration.syntax(), "let declaration is missing its equals sign");
            return None;
        }
        let ports = declaration
            .port_list()
            .map(|ports| {
                ports.ports().children().map(|port| self.lower_assertion_port(port)).collect()
            })
            .unwrap_or_default();
        let expr = self.lower_expr(declaration.expr());
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.let_decls,
            &mut sources.let_decl_srcs,
            LetDecl { name, ports, expr },
            source,
        ))
    }
}
