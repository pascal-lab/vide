use std::ops::Range;

use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const EXPAND_POSTFIX_ID: CodeActionId = CodeActionId {
    name: "expand_postfix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const EXPAND_POSTFIX_LABEL: &str = "Expand postfix increment/decrement";

const EXPAND_PREFIX_ID: CodeActionId = CodeActionId {
    name: "expand_prefix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const EXPAND_PREFIX_LABEL: &str = "Expand prefix increment/decrement";

const POSTFIX_TO_PREFIX_ID: CodeActionId = CodeActionId {
    name: "convert_postfix_to_prefix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const POSTFIX_TO_PREFIX_LABEL: &str = "Convert postfix to prefix increment/decrement";

const POSTFIX_TO_COMPOUND_ID: CodeActionId = CodeActionId {
    name: "convert_postfix_to_compound_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const POSTFIX_TO_COMPOUND_LABEL: &str = "Convert postfix to compound assignment";

const PREFIX_TO_POSTFIX_ID: CodeActionId = CodeActionId {
    name: "convert_prefix_to_postfix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const PREFIX_TO_POSTFIX_LABEL: &str = "Convert prefix to postfix increment/decrement";

const PREFIX_TO_COMPOUND_ID: CodeActionId = CodeActionId {
    name: "convert_prefix_to_compound_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const PREFIX_TO_COMPOUND_LABEL: &str = "Convert prefix to compound assignment";

const COMPOUND_TO_POSTFIX_ID: CodeActionId = CodeActionId {
    name: "convert_compound_to_postfix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const COMPOUND_TO_POSTFIX_LABEL: &str = "Convert compound assignment to postfix";

const COMPOUND_TO_PREFIX_ID: CodeActionId = CodeActionId {
    name: "convert_compound_to_prefix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const COMPOUND_TO_PREFIX_LABEL: &str = "Convert compound assignment to prefix";

const ASSIGNMENT_TO_POSTFIX_ID: CodeActionId = CodeActionId {
    name: "convert_assignment_to_postfix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const ASSIGNMENT_TO_POSTFIX_LABEL: &str = "Convert assignment to postfix";

const ASSIGNMENT_TO_PREFIX_ID: CodeActionId = CodeActionId {
    name: "convert_assignment_to_prefix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const ASSIGNMENT_TO_PREFIX_LABEL: &str = "Convert assignment to prefix";

pub(super) fn expand_postfix_inc_dec(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let mut added = false;
    added |= expand_postfix(collector, ctx).is_some();
    added |= expand_prefix(collector, ctx).is_some();
    added |= convert_postfix_to_prefix(collector, ctx).is_some();
    added |= convert_postfix_to_compound(collector, ctx).is_some();
    added |= convert_prefix_to_postfix(collector, ctx).is_some();
    added |= convert_prefix_to_compound(collector, ctx).is_some();
    added |= convert_compound_to_postfix(collector, ctx).is_some();
    added |= convert_compound_to_prefix(collector, ctx).is_some();
    added |= convert_assignment_to_postfix(collector, ctx).is_some();
    added |= convert_assignment_to_prefix(collector, ctx).is_some();
    added.then_some(())
}

fn expand_postfix(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let expr = postfix_expr(ctx)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let operand = text.get(Range::from(expr.operand().syntax().text_range()?))?;
    let range = expr.syntax().text_range()?;

    collector.add(EXPAND_POSTFIX_ID, EXPAND_POSTFIX_LABEL, range, |builder| {
        builder.replace(range, format!("{operand} = {operand} {} 1", expr.inc_dec.binary_operator()));
    });

    Some(())
}

fn expand_prefix(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let expr = prefix_expr(ctx)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let operand = text.get(Range::from(expr.operand().syntax().text_range()?))?;
    let range = expr.syntax().text_range()?;

    collector.add(EXPAND_PREFIX_ID, EXPAND_PREFIX_LABEL, range, |builder| {
        builder.replace(range, format!("{operand} = {operand} {} 1", expr.inc_dec.binary_operator()));
    });

    Some(())
}

fn convert_postfix_to_prefix(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = postfix_expr(ctx)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let operand = text.get(Range::from(expr.operand().syntax().text_range()?))?;
    let range = expr.syntax().text_range()?;

    collector.add(POSTFIX_TO_PREFIX_ID, POSTFIX_TO_PREFIX_LABEL, range, |builder| {
        builder.replace(range, format!("{}{}", expr.inc_dec.operator(), operand.trim()));
    });

    Some(())
}

fn convert_postfix_to_compound(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = postfix_expr(ctx)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let operand = text.get(Range::from(expr.operand().syntax().text_range()?))?;
    let range = expr.syntax().text_range()?;

    collector.add(POSTFIX_TO_COMPOUND_ID, POSTFIX_TO_COMPOUND_LABEL, range, |builder| {
        builder.replace(range, format!("{} {} 1", operand.trim(), expr.inc_dec.compound_operator()));
    });

    Some(())
}

fn convert_prefix_to_postfix(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = prefix_expr(ctx)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let operand = text.get(Range::from(expr.operand().syntax().text_range()?))?;
    let range = expr.syntax().text_range()?;

    collector.add(PREFIX_TO_POSTFIX_ID, PREFIX_TO_POSTFIX_LABEL, range, |builder| {
        builder.replace(range, format!("{}{}", operand.trim(), expr.inc_dec.operator()));
    });

    Some(())
}

fn convert_prefix_to_compound(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = prefix_expr(ctx)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let operand = text.get(Range::from(expr.operand().syntax().text_range()?))?;
    let range = expr.syntax().text_range()?;

    collector.add(PREFIX_TO_COMPOUND_ID, PREFIX_TO_COMPOUND_LABEL, range, |builder| {
        builder.replace(range, format!("{} {} 1", operand.trim(), expr.inc_dec.compound_operator()));
    });

    Some(())
}

fn convert_compound_to_postfix(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = compound_expr(ctx)?;

    collector.add(COMPOUND_TO_POSTFIX_ID, COMPOUND_TO_POSTFIX_LABEL, expr.range, |builder| {
        builder.replace(expr.range, format!("{}{}", expr.operand.trim(), expr.inc_dec.operator()));
    });

    Some(())
}

fn convert_compound_to_prefix(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = compound_expr(ctx)?;

    collector.add(COMPOUND_TO_PREFIX_ID, COMPOUND_TO_PREFIX_LABEL, expr.range, |builder| {
        builder.replace(expr.range, format!("{}{}", expr.inc_dec.operator(), expr.operand.trim()));
    });

    Some(())
}

fn convert_assignment_to_postfix(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = assignment_expr(ctx)?;

    collector.add(ASSIGNMENT_TO_POSTFIX_ID, ASSIGNMENT_TO_POSTFIX_LABEL, expr.range, |builder| {
        builder.replace(expr.range, format!("{}{}", expr.operand.trim(), expr.inc_dec.operator()));
    });

    Some(())
}

fn convert_assignment_to_prefix(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = assignment_expr(ctx)?;

    collector.add(ASSIGNMENT_TO_PREFIX_ID, ASSIGNMENT_TO_PREFIX_LABEL, expr.range, |builder| {
        builder.replace(expr.range, format!("{}{}", expr.inc_dec.operator(), expr.operand.trim()));
    });

    Some(())
}

struct UnaryIncDec<'a> {
    syntax: syntax::SyntaxNode<'a>,
    operand: ast::Expression<'a>,
    inc_dec: IncDec,
}

impl<'a> UnaryIncDec<'a> {
    fn syntax(&self) -> syntax::SyntaxNode<'a> {
        self.syntax
    }

    fn operand(&self) -> ast::Expression<'a> {
        self.operand
    }
}

struct CompoundIncDec {
    range: utils::text_edit::TextRange,
    operand: String,
    inc_dec: IncDec,
}

struct AssignmentIncDec {
    range: utils::text_edit::TextRange,
    operand: String,
    inc_dec: IncDec,
}

fn postfix_expr<'a>(ctx: &'a CodeActionCtx) -> Option<UnaryIncDec<'a>> {
    let expr = ctx.find_node_at_offset::<ast::PostfixUnaryExpression>()?;
    let inc_dec = IncDec::from_operator(&expr.operator_token()?.value_text().to_string())?;
    Some(UnaryIncDec { syntax: expr.syntax(), operand: expr.operand(), inc_dec })
}

fn prefix_expr<'a>(ctx: &'a CodeActionCtx) -> Option<UnaryIncDec<'a>> {
    let expr = ctx.find_node_at_offset::<ast::PrefixUnaryExpression>()?;
    let inc_dec = prefix_inc_dec(expr)?;
    Some(UnaryIncDec { syntax: expr.syntax(), operand: expr.operand(), inc_dec })
}

fn compound_expr(ctx: &CodeActionCtx) -> Option<CompoundIncDec> {
    let expr = ctx.find_node_at_offset::<ast::BinaryExpression>()?;
    let inc_dec = IncDec::from_compound_operator(&expr.operator_token()?.value_text().to_string())?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let right = text.get(Range::from(expr.right().syntax().text_range()?))?;
    if right.trim() != "1" {
        return None;
    }

    Some(CompoundIncDec {
        range: expr.syntax().text_range()?,
        operand: text.get(Range::from(expr.left().syntax().text_range()?))?.to_owned(),
        inc_dec,
    })
}

fn assignment_expr(ctx: &CodeActionCtx) -> Option<AssignmentIncDec> {
    let expr = ctx.find_node_at_offset::<ast::BinaryExpression>()?;
    if expr.operator_token()?.value_text().to_string() != "=" {
        return None;
    }

    let right = expr.right().as_binary_expression()?;
    let inc_dec = IncDec::from_binary_operator(&right.operator_token()?.value_text().to_string())?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let left = text.get(Range::from(expr.left().syntax().text_range()?))?;
    let right_left = text.get(Range::from(right.left().syntax().text_range()?))?;
    let right_right = text.get(Range::from(right.right().syntax().text_range()?))?;
    if left.trim() != right_left.trim() || right_right.trim() != "1" {
        return None;
    }

    Some(AssignmentIncDec { range: expr.syntax().text_range()?, operand: left.to_owned(), inc_dec })
}

fn prefix_inc_dec(expr: ast::PrefixUnaryExpression<'_>) -> Option<IncDec> {
    if expr.as_unary_preincrement_expression().is_some() {
        Some(IncDec::Increment)
    } else if expr.as_unary_predecrement_expression().is_some() {
        Some(IncDec::Decrement)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
enum IncDec {
    Increment,
    Decrement,
}

impl IncDec {
    fn from_operator(op: &str) -> Option<Self> {
        match op {
            "++" => Some(Self::Increment),
            "--" => Some(Self::Decrement),
            _ => None,
        }
    }

    fn from_compound_operator(op: &str) -> Option<Self> {
        match op {
            "+=" => Some(Self::Increment),
            "-=" => Some(Self::Decrement),
            _ => None,
        }
    }

    fn from_binary_operator(op: &str) -> Option<Self> {
        match op {
            "+" => Some(Self::Increment),
            "-" => Some(Self::Decrement),
            _ => None,
        }
    }

    fn operator(self) -> &'static str {
        match self {
            Self::Increment => "++",
            Self::Decrement => "--",
        }
    }

    fn compound_operator(self) -> &'static str {
        match self {
            Self::Increment => "+=",
            Self::Decrement => "-=",
        }
    }

    fn binary_operator(self) -> &'static str {
        match self {
            Self::Increment => "+",
            Self::Decrement => "-",
        }
    }
}
