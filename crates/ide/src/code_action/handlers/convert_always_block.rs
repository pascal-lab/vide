use std::ops::Range;

use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ALWAYS_TO_COMB_ID: CodeActionId = CodeActionId {
    name: "convert_always_to_always_comb",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const ALWAYS_TO_COMB_LABEL: &str = "Convert to always_comb";

const ALWAYS_TO_FF_ID: CodeActionId = CodeActionId {
    name: "convert_always_to_always_ff",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const ALWAYS_TO_FF_LABEL: &str = "Convert to always_ff";

const ALWAYS_COMB_TO_ALWAYS_ID: CodeActionId = CodeActionId {
    name: "convert_always_comb_to_always",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const ALWAYS_COMB_TO_ALWAYS_LABEL: &str = "Convert to always @(*)";

const ALWAYS_FF_TO_ALWAYS_ID: CodeActionId = CodeActionId {
    name: "convert_always_ff_to_always",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const ALWAYS_FF_TO_ALWAYS_LABEL: &str = "Convert to always @(...)";

pub(super) fn convert_always_block(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let proc = ctx.find_node_at_offset::<ast::ProceduralBlock>()?;
    let keyword = proc.keyword()?.text_range_in(proc.syntax())?;
    let target = proc.syntax().text_range()?;
    let mut allowed_ranges = vec![keyword];

    match proc {
        ast::ProceduralBlock::AlwaysBlock(_) => {
            let timing_stmt = proc.statement().as_timing_control_statement()?;
            let timing = timing_stmt.timing_control();
            allowed_ranges.push(timing.syntax().text_range()?);
            if !range_intersects_any(ctx.range(), &allowed_ranges) {
                return None;
            }

            if timing.as_implicit_event_control().is_some() {
                let stmt_range = timing_stmt.statement().syntax().text_range()?;
                let text = ctx.sema().db.file_text(ctx.file_id());
                let stmt_text = text.get(Range::from(stmt_range))?;
                collector.add(ALWAYS_TO_COMB_ID, ALWAYS_TO_COMB_LABEL, target, |builder| {
                    builder.replace(keyword, "always_comb");
                    builder
                        .replace(timing_stmt.syntax().text_range().unwrap(), stmt_text.to_owned());
                });
            }

            if edge_sensitive_timing_control(timing) {
                collector.add(ALWAYS_TO_FF_ID, ALWAYS_TO_FF_LABEL, target, |builder| {
                    builder.replace(keyword, "always_ff");
                });
            }

            Some(())
        }
        ast::ProceduralBlock::AlwaysCombBlock(_) => {
            if !range_intersects_any(ctx.range(), &allowed_ranges) {
                return None;
            }

            collector.add(
                ALWAYS_COMB_TO_ALWAYS_ID,
                ALWAYS_COMB_TO_ALWAYS_LABEL,
                target,
                |builder| {
                    builder.replace(keyword, "always");
                    builder.insert(keyword.end(), " @(*)");
                },
            )
        }
        ast::ProceduralBlock::AlwaysFFBlock(_) => {
            let timing_stmt = proc.statement().as_timing_control_statement()?;
            allowed_ranges.push(timing_stmt.timing_control().syntax().text_range()?);
            if !range_intersects_any(ctx.range(), &allowed_ranges) {
                return None;
            }

            if !edge_sensitive_timing_control(timing_stmt.timing_control()) {
                return None;
            }

            collector.add(ALWAYS_FF_TO_ALWAYS_ID, ALWAYS_FF_TO_ALWAYS_LABEL, target, |builder| {
                builder.replace(keyword, "always");
            })
        }
        _ => None,
    }
}

fn range_intersects_any(
    range: utils::text_edit::TextRange,
    allowed_ranges: &[utils::text_edit::TextRange],
) -> bool {
    allowed_ranges.iter().any(|allowed| range_intersects(range, *allowed))
}

fn range_intersects(lhs: utils::text_edit::TextRange, rhs: utils::text_edit::TextRange) -> bool {
    if lhs.is_empty() {
        rhs.contains(lhs.start())
    } else {
        lhs.start() < rhs.end() && rhs.start() < lhs.end()
    }
}

fn edge_sensitive_timing_control(timing: ast::TimingControl<'_>) -> bool {
    timing
        .as_event_control_with_expression()
        .is_some_and(|control| edge_sensitive_event_expr(control.expr()))
}

fn edge_sensitive_event_expr(expr: ast::EventExpression<'_>) -> bool {
    match expr {
        ast::EventExpression::ParenthesizedEventExpression(expr) => {
            edge_sensitive_event_expr(expr.expr())
        }
        ast::EventExpression::BinaryEventExpression(expr) => {
            edge_sensitive_event_expr(expr.left()) && edge_sensitive_event_expr(expr.right())
        }
        ast::EventExpression::SignalEventExpression(expr) => expr.edge().is_some(),
    }
}
