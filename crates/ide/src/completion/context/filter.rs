use syntax::{SyntaxNode, SyntaxNodeExt, TokenKind, TriviaKind};
use utils::text_edit::TextSize;

pub fn should_complete(syntax: SyntaxNode, position: TextSize) -> bool {
    let pos: usize = position.into();

    if is_inside_comment(syntax, pos) {
        return false;
    }

    let token = syntax.token_at_offset(position).left_biased();

    if token.is_none() {
        return true;
    }

    let token = token.unwrap();
    let kind = token.tok.kind();

    if is_string_token(kind) || is_numeric_literal(kind) {
        return false;
    }

    true
}

fn is_inside_comment(syntax: SyntaxNode, pos: usize) -> bool {
    fn check_node(node: SyntaxNode, pos: usize) -> bool {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if let Some(token) = child.as_token() {
                    for ((start, end), trivia) in token.trivias_with_loc() {
                        if matches!(
                            trivia.kind(),
                            TriviaKind::LINE_COMMENT | TriviaKind::BLOCK_COMMENT
                        ) && start <= pos
                            && pos <= end
                        {
                            return true;
                        }
                    }
                } else if let Some(child_node) = child.as_node()
                    && check_node(child_node, pos)
                {
                    return true;
                }
            }
        }
        false
    }

    check_node(syntax, pos)
}

fn is_string_token(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::STRING_LITERAL)
}

fn is_numeric_literal(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::INTEGER_LITERAL
            | TokenKind::INTEGER_BASE
            | TokenKind::REAL_LITERAL
            | TokenKind::UNBASED_UNSIZED_LITERAL
            | TokenKind::TIME_LITERAL
    )
}
