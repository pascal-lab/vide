use hir::{container::InContainer, hir_def::expr::Expr, semantics::Semantics};
use ide_db::root_db::RootDb;
use span::{FilePosition, RangeInfo};
use syntax::{
    SyntaxNodeExt, SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    token::TokenKindExt,
};
use utils::get::GetRef;

use crate::{definitions::DefinitionClass, markup::Markup, render};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverFormat {
    Markdown,
    PlainText,
}

#[derive(Debug, Clone)]
pub struct HoverConfig {
    pub format: HoverFormat,
}

pub(crate) fn hover(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    _config: HoverConfig,
) -> Option<RangeInfo<Markup>> {
    let sema = Semantics::new(db);
    let root = sema.parse_root(file_id);
    let token = root.token_at_offset(offset).pick_bext_token(token_precedence)?;

    let res = handle_literal(&sema, token).or_else(|| handle_definition(&sema, token))?;
    Some(RangeInfo::new(token.text_range()?, res))
}

pub(crate) fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_literal() => 3,
        _ => 1,
    }
}

fn handle_literal(
    sema: &Semantics<RootDb>,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Markup> {
    if !tok.kind().is_literal() {
        return None;
    }

    let expr = ast::Expression::cast(parent)?;
    let InContainer { value: expr_id, cont_id } = sema.resolve_expr(expr);
    let container = cont_id.to_container(sema.db);
    let Expr::Literal(literal) = container.get(expr_id) else {
        return None;
    };

    render::render_literal(literal)
}

fn handle_definition(sema: &Semantics<RootDb>, tp: SyntaxTokenWithParent) -> Option<Markup> {
    let def = DefinitionClass::resolve(sema, tp)?;
    let mut res = Markup::new();

    match def {
        DefinitionClass::Definition(def) => {
            res.merge(render::render_definition(sema, def));
        }
        DefinitionClass::PortConnShorthand { port, local } => {
            res.new_subsection("Port");
            res.merge(render::render_definition(sema, port));
            res.horizontal_line();
            res.new_subsection("Local");
            res.merge(render::render_definition(sema, local));
        }
    }

    Some(res)
}
