use std::{cmp::Ordering, ops::Range};

use hir::base_db::source_db::SourceDb;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ID: CodeActionId = CodeActionId {
    name: "remove_parentheses",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};

// Assist: remove_parentheses
//
// This removes parentheses when they are redundant for the surrounding expression.
//
// ```
// assign y = $0(a + b) + c;
// ```
// ->
// ```
// assign y = a + b + c;
// ```
pub(super) fn remove_parentheses(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let parens = ctx.find_node_at_offset::<ast::ParenthesizedExpression>()?;
    let range = parens.syntax().text_range()?;
    let left = parens.open_paren()?.text_range_in(parens.syntax())?;
    let right = parens.close_paren()?.text_range_in(parens.syntax())?;
    if !left.contains_range(ctx.range()) && !right.contains_range(ctx.range()) {
        return None;
    }

    let expr = parens.expression();
    let parent = parens.syntax().parent()?;
    if parentheses_are_required(parens, expr, parent) {
        return None;
    }

    let expr_range = expr.syntax().text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let inner = text.get(Range::from(expr_range))?.to_owned();
    collector.add(ID, "Remove redundant parentheses", range, |builder| {
        builder.replace(range, inner);
    })
}

fn parentheses_are_required(
    parens: ast::ParenthesizedExpression<'_>,
    expr: ast::Expression<'_>,
    parent: syntax::SyntaxNode<'_>,
) -> bool {
    if ast::ParenthesizedExpression::cast(parent).is_some() {
        return false;
    }

    if matches!(parent.kind(), SyntaxKind::MEMBER_ACCESS_EXPRESSION | SyntaxKind::SCOPED_NAME) {
        return true;
    }

    let Some(parent_binary) = ast::BinaryExpression::cast(parent) else {
        return ast::Expression::cast(parent).is_some_and(|_| {
            expr.as_binary_expression().is_some() || expr.as_conditional_expression().is_some()
        });
    };
    let Some(child_binary) = expr.as_binary_expression() else {
        return false;
    };

    let (Some(parent_prec), Some(child_prec)) =
        (binary_precedence(parent_binary), binary_precedence(child_binary))
    else {
        return true;
    };

    match child_prec.cmp(&parent_prec) {
        Ordering::Greater => false,
        Ordering::Less => true,
        Ordering::Equal => {
            let same_associative_op = parent_binary
                .operator_token()
                .zip(child_binary.operator_token())
                .is_some_and(|(parent_op, child_op)| {
                    parent_op.kind() == child_op.kind()
                        && associative_binary_operator(parent_op.kind())
                });
            !(parent_binary.left().syntax() == parens.syntax() && same_associative_op)
        }
    }
}

fn associative_binary_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::PLUS
            | TokenKind::STAR
            | TokenKind::DOUBLE_AND
            | TokenKind::DOUBLE_OR
            | TokenKind::AND
            | TokenKind::OR
            | TokenKind::XOR
            | TokenKind::TILDE_XOR
            | TokenKind::XOR_TILDE
    )
}

fn binary_precedence(expr: ast::BinaryExpression<'_>) -> Option<u8> {
    let kind = expr.operator_token()?.kind();
    Some(match kind {
        TokenKind::DOUBLE_STAR => 12,
        TokenKind::STAR | TokenKind::SLASH | TokenKind::PERCENT => 11,
        TokenKind::PLUS | TokenKind::MINUS => 10,
        TokenKind::LEFT_SHIFT
        | TokenKind::RIGHT_SHIFT
        | TokenKind::TRIPLE_LEFT_SHIFT
        | TokenKind::TRIPLE_RIGHT_SHIFT => 9,
        TokenKind::LESS_THAN_EQUALS
            if expr.syntax().kind() == SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION =>
        {
            1
        }
        TokenKind::GREATER_THAN
        | TokenKind::GREATER_THAN_EQUALS
        | TokenKind::LESS_THAN
        | TokenKind::LESS_THAN_EQUALS => 8,
        TokenKind::DOUBLE_EQUALS
        | TokenKind::EXCLAMATION_EQUALS
        | TokenKind::TRIPLE_EQUALS
        | TokenKind::EXCLAMATION_DOUBLE_EQUALS
        | TokenKind::DOUBLE_EQUALS_QUESTION
        | TokenKind::EXCLAMATION_EQUALS_QUESTION => 7,
        TokenKind::AND => 6,
        TokenKind::XOR | TokenKind::TILDE_XOR | TokenKind::XOR_TILDE => 5,
        TokenKind::OR => 4,
        TokenKind::DOUBLE_AND => 3,
        TokenKind::DOUBLE_OR => 2,
        TokenKind::EQUALS
        | TokenKind::PLUS_EQUAL
        | TokenKind::MINUS_EQUAL
        | TokenKind::STAR_EQUAL
        | TokenKind::SLASH_EQUAL
        | TokenKind::PERCENT_EQUAL
        | TokenKind::AND_EQUAL
        | TokenKind::OR_EQUAL
        | TokenKind::XOR_EQUAL
        | TokenKind::LEFT_SHIFT_EQUAL
        | TokenKind::RIGHT_SHIFT_EQUAL
        | TokenKind::TRIPLE_LEFT_SHIFT_EQUAL
        | TokenKind::TRIPLE_RIGHT_SHIFT_EQUAL => 1,
        _ => return None,
    })
}
