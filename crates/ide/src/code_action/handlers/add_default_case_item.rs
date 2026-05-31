use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent, newline_style,
};

const ID: CodeActionId =
    CodeActionId { name: "add_default_case_item", kind: CodeActionKind::Generate, repair: None };
const LABEL: &str = "Add default case item";

pub(super) fn add_default_case_item(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let case = ctx.find_node_at_offset::<ast::CaseStatement>()?;
    if case.items().children().any(|item| item.as_default_case_item().is_some()) {
        return None;
    }

    let endcase = case.endcase()?.text_range_in(case.syntax())?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let newline = newline_style(&text);
    let end_indent = line_indent(&text, endcase.start());
    let item_indent = case
        .items()
        .children()
        .filter_map(|item| item.syntax().text_range())
        .next()
        .map(|range| line_indent(&text, range.start()))
        .filter(|indent| !indent.is_empty())
        .unwrap_or_else(|| format!("{end_indent}    "));

    let before_end = &text[..usize::from(endcase.start())];
    let prefix = if before_end.ends_with('\n') { "" } else { newline };
    let insertion = format!("{prefix}{item_indent}default: ;{newline}{end_indent}");
    collector.add(ID, LABEL, endcase, |builder| {
        builder.insert(endcase.start(), insertion);
    });

    Some(())
}
