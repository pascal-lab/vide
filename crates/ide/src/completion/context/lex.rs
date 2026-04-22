use syntax::{SyntaxAncestors, SyntaxNodeExt, TokenKind, has_text_range::HasTextRange};
use utils::line_index::TextSize;

use super::{LexContext, caret::CaretSnapshot};

pub(super) fn detect_lex_context(caret: &CaretSnapshot<'_>) -> LexContext {
    if is_inside_string_literal(caret) {
        return LexContext::StringLiteral;
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

fn is_inside_string_literal(caret: &CaretSnapshot<'_>) -> bool {
    let tok = caret.root.token_at_offset(caret.offset).left_biased();
    tok.is_some_and(|tp| {
        tp.kind() == TokenKind::STRING_LITERAL
            && tp.text_range().is_some_and(|r| r.contains(caret.offset))
    })
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
    SyntaxAncestors::start_from(node).any(|n| is_preproc_directive_kind(n.kind()))
}

fn is_preproc_directive_kind(kind: syntax::SyntaxKind) -> bool {
    use syntax::SyntaxKind;
    matches!(
        kind,
        SyntaxKind::BEGIN_KEYWORDS_DIRECTIVE
            | SyntaxKind::CELL_DEFINE_DIRECTIVE
            | SyntaxKind::DEFAULT_DECAY_TIME_DIRECTIVE
            | SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE
            | SyntaxKind::DEFAULT_TRIREG_STRENGTH_DIRECTIVE
            | SyntaxKind::DEFINE_DIRECTIVE
            | SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE
            | SyntaxKind::DELAY_MODE_PATH_DIRECTIVE
            | SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE
            | SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE
            | SyntaxKind::ELSE_DIRECTIVE
            | SyntaxKind::ELS_IF_DIRECTIVE
            | SyntaxKind::END_CELL_DEFINE_DIRECTIVE
            | SyntaxKind::END_IF_DIRECTIVE
            | SyntaxKind::END_KEYWORDS_DIRECTIVE
            | SyntaxKind::END_PROTECT_DIRECTIVE
            | SyntaxKind::END_PROTECTED_DIRECTIVE
            | SyntaxKind::IF_DEF_DIRECTIVE
            | SyntaxKind::IF_N_DEF_DIRECTIVE
            | SyntaxKind::INCLUDE_DIRECTIVE
            | SyntaxKind::LINE_DIRECTIVE
            | SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE
            | SyntaxKind::PRAGMA_DIRECTIVE
            | SyntaxKind::PROTECT_DIRECTIVE
            | SyntaxKind::PROTECTED_DIRECTIVE
            | SyntaxKind::RESET_ALL_DIRECTIVE
            | SyntaxKind::TIME_SCALE_DIRECTIVE
            | SyntaxKind::UNCONNECTED_DRIVE_DIRECTIVE
            | SyntaxKind::UNDEF_DIRECTIVE
            | SyntaxKind::UNDEFINE_ALL_DIRECTIVE
            | SyntaxKind::BIND_DIRECTIVE
    )
}
