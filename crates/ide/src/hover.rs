use hir::{
    base_db::source_db::SourceDb, container::InContainer, file::HirFileId, hir_def::expr::Expr,
    semantics::Semantics,
};
use syntax::{
    SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    token::TokenKindExt,
};
use utils::{
    get::GetRef,
    line_index::{TextRange, TextSize},
    uniq_vec::UniqVec,
};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    hover::{
        include::render_include_hover,
        macro_hover::{render_macro_hover_target, with_expanded_macro_hover},
    },
    markup::{Markup, inline_code},
    render,
    semantic_target::{SemanticTarget, TargetIntent, TargetResolution, resolve_semantic_target},
    source_targets::SourceTarget,
};

mod include;
mod macro_hover;

#[cfg(test)]
use macro_hover::macro_expansion_hover_text;

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
    let parsed_file = sema.parse_file(file_id);
    let target = resolve_semantic_target(
        db,
        file_id,
        offset,
        parsed_file.root(),
        TargetIntent::Describe,
        token_precedence,
    );
    render_hover_target(db, file_id, offset, &sema, target)
}

fn render_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    sema: &Semantics<RootDb>,
    target: TargetResolution<'_>,
) -> Option<RangeInfo<Markup>> {
    match target.for_hover()? {
        SemanticTarget::PreprocMacro(target) => {
            render_macro_hover_target(db, file_id, offset, target)
        }
        SemanticTarget::Include(includes) => render_include_hover(db, includes),
        SemanticTarget::Source(target) => {
            let hover = hover_for_source_target(sema, file_id.into(), target)?;
            Some(with_expanded_macro_hover(db, file_id, offset, hover))
        }
    }
}

fn hover_for_source_target(
    sema: &Semantics<RootDb>,
    hir_file_id: HirFileId,
    target: SourceTarget<'_>,
) -> Option<RangeInfo<Markup>> {
    let (range, tokens) = target.into_parts();
    hover_for_token_selection(sema, hir_file_id, range, tokens)
}

fn hover_for_token_selection(
    sema: &Semantics<RootDb>,
    hir_file_id: HirFileId,
    range: TextRange,
    tokens: Vec<SyntaxTokenWithParent<'_>>,
) -> Option<RangeInfo<Markup>> {
    let markups = tokens
        .into_iter()
        .filter_map(|token| hover_for_token(sema, hir_file_id, token))
        .collect::<Vec<_>>();
    let res = merge_hover_results(markups)?;
    Some(RangeInfo::new(range, res))
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
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Markup> {
    if !tok.kind().is_literal() {
        return None;
    }

    let expr = ast::Expression::cast(parent)?;
    let InContainer { value: expr_id, cont_id } = sema.resolve_expr(file_id, expr)?;
    let container = cont_id.to_container(sema.db);
    let Expr::Literal(literal) = container.get(expr_id) else {
        return None;
    };

    render::render_literal(literal)
}

fn hover_for_token(
    sema: &Semantics<RootDb>,
    file_id: HirFileId,
    token: SyntaxTokenWithParent,
) -> Option<Markup> {
    handle_literal(sema, file_id, token).or_else(|| handle_definition(sema, file_id, token))
}

fn merge_hover_results(markups: Vec<Markup>) -> Option<Markup> {
    let mut unique = UniqVec::<Markup, Markup>::default();
    for markup in markups {
        unique.push_unique(markup);
    }

    let mut iter = unique.into_vec().into_iter();
    let mut res = iter.next()?;
    for markup in iter {
        res.horizontal_line();
        res.merge(markup);
    }
    Some(res)
}

fn handle_definition(
    sema: &Semantics<RootDb>,
    file_id: HirFileId,
    tp: SyntaxTokenWithParent,
) -> Option<Markup> {
    let token_text = token_text(sema.db, file_id, &tp);
    let def = DefinitionClass::resolve(sema, file_id, tp)?;
    let anchor_file_id = file_id.file_id();
    let mut res = Markup::new();

    match def {
        DefinitionClass::Definition(def) => {
            res.merge(render::render_definition(sema, def, anchor_file_id));
        }
        DefinitionClass::PortConnShorthand { port, local } => {
            res.title("Port connection shorthand");
            res.section("Port");
            res.merge(render::render_definition(sema, port, anchor_file_id));
            res.section("Local");
            res.merge(render::render_definition(sema, local, anchor_file_id));
        }
        DefinitionClass::Ambiguous(definitions) => {
            let token_text = token_text.unwrap_or_else(|| "reference".to_string());
            let candidate_count = definitions.len();
            res.title(&format!("Module reference {}", inline_code(&token_text)));
            res.push_with_code_fence(&token_text);
            res.metadata_line(&format!(
                "ambiguous reference, {candidate_count} candidate{}",
                if candidate_count == 1 { "" } else { "s" }
            ));
            res.section("Candidates");
            for (idx, definition) in definitions.into_iter().enumerate() {
                if idx > 0 && !res.as_str().ends_with('\n') {
                    res.print("\n");
                }
                res.merge(render::render_definition_location(sema, definition, anchor_file_id));
            }
        }
    }

    Some(res)
}

fn token_text(
    db: &RootDb,
    file_id: HirFileId,
    token: &SyntaxTokenWithParent<'_>,
) -> Option<String> {
    let range = token.text_range()?;
    let source = db.file_text(file_id.file_id());
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    source.get(start..end).map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn macro_expansion_hover_text_matrix() {
        let mut report = String::new();

        for (name, text) in [
            (
                "dedents common indentation",
                "\n    always_ff @(posedge clk) begin\n      q <= 1;\n    end\n",
            ),
            ("removes single-line callsite indent", "  logic generated;"),
        ] {
            writeln!(&mut report, "{name}:").unwrap();
            writeln!(&mut report, "{}", macro_expansion_hover_text(text)).unwrap();
        }

        insta::assert_snapshot!(report);
    }
}
