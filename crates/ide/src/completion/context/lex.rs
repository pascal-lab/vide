use syntax::{SyntaxAncestors, SyntaxNodeExt, has_text_range::HasTextRange, token::TokenKindExt};
use utils::line_index::TextSize;

use super::{LexContext, caret::CaretSnapshot};
use crate::completion::directives::is_directive_kind;

pub(super) fn detect_lex_context(caret: &CaretSnapshot<'_>) -> LexContext {
    if is_inside_literal(caret) {
        return LexContext::Literal;
    }

    if is_at_literal_boundary(caret) {
        return LexContext::Literal;
    }

    if let Some(kind) = trivia_kind_at_caret_offset(caret) {
        return match kind {
            syntax::Trivia![lc] => LexContext::LineComment,
            syntax::Trivia![bc] => LexContext::BlockComment,
            syntax::Trivia!["`"] => LexContext::PreprocDirective,
            _ => LexContext::Code,
        };
    }

    if is_inside_preproc_directive_trivia(caret) {
        return LexContext::PreprocDirective;
    }

    if is_inside_preproc_directive_node(caret) {
        return LexContext::PreprocDirective;
    }

    LexContext::Code
}

fn trivia_kind_at_caret_offset(caret: &CaretSnapshot<'_>) -> Option<syntax::TriviaKind> {
    let kind = caret.root.trivia_kind_at_offset(caret.offset);
    if !matches!(kind, Some(syntax::Trivia![eol]) | None) {
        return kind;
    }

    if caret.offset == TextSize::new(0) {
        return None;
    }

    let prev = caret.offset - TextSize::new(1);
    let kind = caret.root.trivia_kind_at_offset(prev)?;
    (kind == syntax::Trivia![lc]).then_some(kind)
}

fn is_inside_literal(caret: &CaretSnapshot<'_>) -> bool {
    let tok = caret.root.token_at_offset(caret.offset).left_biased();
    tok.is_some_and(|tp| {
        tp.kind().is_literal() && tp.text_range().is_some_and(|r| r.contains(caret.offset))
    })
}

fn is_at_literal_boundary(caret: &CaretSnapshot<'_>) -> bool {
    let Some(prev) = caret.root.token_before_offset(caret.offset) else {
        return false;
    };
    let Some(range) = prev.text_range() else {
        return false;
    };
    if range.end() != caret.offset {
        return false;
    }

    if prev.kind().is_literal() {
        return true;
    }

    let Some(before_prev) = caret.root.token_before_offset(range.start()) else {
        return false;
    };
    let is_after_integer_size = before_prev.kind() == syntax::TokenKind::INTEGER_LITERAL
        && before_prev.text_range().is_some_and(|before_range| before_range.end() == range.start());
    if !is_after_integer_size {
        return false;
    }

    matches!(prev.kind(), syntax::TokenKind::INTEGER_BASE | syntax::Token!["'"])
}

fn is_inside_preproc_directive_trivia(caret: &CaretSnapshot<'_>) -> bool {
    fn token_has_covering_directive_trivia(
        tok: syntax::SyntaxTokenWithParent<'_>,
        offset: TextSize,
    ) -> bool {
        tok.tok.trivias().any(|trivia| {
            if trivia.kind() != syntax::Trivia!["`"] {
                return false;
            }

            let Some(node) = trivia.syntax() else {
                return false;
            };
            node.text_range().is_some_and(|range| range.contains(offset) || range.end() == offset)
        })
    }

    if caret
        .root
        .token_after_or_at_offset(caret.offset)
        .is_some_and(|tok| token_has_covering_directive_trivia(tok, caret.offset))
    {
        return true;
    }

    caret
        .root
        .token_before_offset(caret.offset)
        .is_some_and(|tok| token_has_covering_directive_trivia(tok, caret.offset))
}

fn is_inside_preproc_directive_node(caret: &CaretSnapshot<'_>) -> bool {
    let Some(node) = caret.covering_node() else {
        return false;
    };
    SyntaxAncestors::start_from(node).any(|n| is_directive_kind(n.kind()))
}
