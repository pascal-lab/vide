use syntax::{SyntaxAncestors, SyntaxNodeExt, has_text_range::HasTextRange, token::TokenKindExt};
use utils::line_index::TextSize;

use super::{LexContext, caret::CaretSnapshot};

pub(super) fn detect_lex_context(caret: &CaretSnapshot<'_>) -> LexContext {
    if is_inside_literal(caret) {
        return LexContext::Literal;
    }

    if is_typing_based_literal(caret) {
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

fn is_typing_based_literal(caret: &CaretSnapshot<'_>) -> bool {
    let Some(before) = line_text_before_caret(caret) else { return false };
    let Some(quote_idx) = before.rfind('\'') else {
        return false;
    };

    let (size, after_quote) = before.split_at(quote_idx);
    if size.is_empty() || !size.bytes().next_back().is_some_and(|b| b.is_ascii_digit()) {
        return false;
    }
    if !size
        .bytes()
        .rev()
        .take_while(|b| b.is_ascii_digit() || *b == b'_')
        .any(|b| b.is_ascii_digit())
    {
        return false;
    }

    let after_quote = &after_quote[1..];
    let mut chars = after_quote.chars();
    let Some(base) = chars.next() else {
        return true;
    };
    if !matches!(base, 'b' | 'B' | 'o' | 'O' | 'd' | 'D' | 'h' | 'H' | 's' | 'S') {
        return false;
    }
    let digits = if matches!(base, 's' | 'S') {
        let Some(base) = chars.next() else {
            return true;
        };
        if !matches!(base, 'b' | 'B' | 'o' | 'O' | 'd' | 'D' | 'h' | 'H') {
            return false;
        }
        chars.as_str()
    } else {
        chars.as_str()
    };

    digits
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'x' | b'X' | b'z' | b'Z' | b'?'))
}

fn line_text_before_caret(caret: &CaretSnapshot<'_>) -> Option<String> {
    let mut tokens = Vec::new();
    let mut prev_offset = caret.offset;
    while prev_offset > TextSize::new(0) {
        let tok = caret.root.token_before_offset(prev_offset)?;
        let range = tok.text_range()?;
        let text = tok.tok.raw_text().to_string();
        if let Some(line_start) = text.rfind('\n') {
            tokens.push(text[line_start + 1..].to_owned());
            break;
        }
        tokens.push(text);
        prev_offset = range.start();
    }

    tokens.reverse();
    Some(tokens.concat())
}

fn is_inside_literal(caret: &CaretSnapshot<'_>) -> bool {
    let tok = caret.root.token_at_offset(caret.offset).left_biased();
    tok.is_some_and(|tp| {
        tp.kind().is_literal() && tp.text_range().is_some_and(|r| r.contains(caret.offset))
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
