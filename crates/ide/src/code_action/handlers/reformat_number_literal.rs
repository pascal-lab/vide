use std::ops::Range;

use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextRange;

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ID: CodeActionId = CodeActionId {
    name: "reformat_number_literal",
    kind: CodeActionKind::RefactorInline,
    repair: None,
};
const MIN_NUMBER_OF_DIGITS_TO_FORMAT: usize = 5;

// Assist: reformat_number_literal
//
// This adds digit separators to long integer literals or removes existing digit
// separators.
//
// ```
// localparam int value = 10000$0;
// ```
// ->
// ```
// localparam int value = 10_000;
// ```
pub(super) fn reformat_number_literal(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let text = ctx.sema().db.file_text(ctx.file_id());
    let (raw, prefix, digits, group_size, range) = selected_integer_literal(ctx, &text)?;

    if digits.contains('_') {
        let replacement = raw.replace('_', "");
        return collector.add(ID, "Remove digit separators", range, |builder| {
            builder.replace(range, replacement);
        });
    }

    if digits.chars().count() < MIN_NUMBER_OF_DIGITS_TO_FORMAT {
        return None;
    }

    let replacement = format!("{}{}", prefix, add_group_separators(digits, group_size));
    let label = format!("Convert {raw} to {replacement}");
    collector.add(ID, label, range, |builder| {
        builder.replace(range, replacement);
    })
}

fn selected_integer_literal<'a>(
    ctx: &CodeActionCtx<'_>,
    text: &'a str,
) -> Option<(&'a str, &'a str, &'a str, usize, TextRange)> {
    if let Some(expr) = ctx.find_node_at_offset::<ast::IntegerVectorExpression>() {
        let range = expr.syntax().text_range()?;
        let raw = text.get(Range::from(range))?;
        return parse_based_literal(raw, range);
    }

    let literal = ctx.find_node_at_offset::<ast::LiteralExpression>()?;
    let ast::LiteralExpression::IntegerLiteralExpression(integer) = literal else {
        return None;
    };
    let range = integer.text_range()?;
    let raw = text.get(Range::from(range))?;
    Some((raw, "", raw, 3, range))
}

fn parse_based_literal(
    raw: &str,
    range: TextRange,
) -> Option<(&str, &str, &str, usize, TextRange)> {
    let apostrophe = raw.find('\'')?;
    let after_quote = raw.get(apostrophe + 1..)?;
    let (sign_len, rest) = match after_quote.as_bytes().first().copied() {
        Some(b's' | b'S') => (1usize, after_quote.get(1..)?),
        _ => (0usize, after_quote),
    };
    let base = rest.as_bytes().first().copied()?;
    let group_size = match base.to_ascii_lowercase() {
        b'b' => 4,
        b'o' => 3,
        b'd' => 3,
        b'h' => 4,
        _ => return None,
    };
    let digits_start = apostrophe + 1 + sign_len + 1;
    let digits = raw.get(digits_start..)?;
    Some((raw, raw.get(..digits_start)?, digits, group_size, range))
}

fn add_group_separators(digits: &str, group_size: usize) -> String {
    let clean: Vec<char> = digits.chars().filter(|ch| *ch != '_').collect();
    let mut buf = String::with_capacity(clean.len() + clean.len() / group_size);
    for (idx, ch) in clean.iter().rev().enumerate() {
        if idx != 0 && idx % group_size == 0 {
            buf.push('_');
        }
        buf.push(*ch);
    }
    buf.chars().rev().collect()
}
