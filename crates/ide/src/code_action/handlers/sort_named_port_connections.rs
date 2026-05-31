use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::text_edit::TextRange;

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent, newline_style,
    text_at,
};

const ID: CodeActionId = CodeActionId {
    name: "sort_named_port_connections",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const LABEL: &str = "Sort named port connections";

pub(super) fn sort_named_port_connections(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let instance = ctx.find_node_at_offset::<ast::HierarchicalInstance>()?;
    let open = instance.open_paren()?.text_range_in(instance.syntax())?;
    let close = instance.close_paren()?.text_range_in(instance.syntax())?;

    let text = ctx.sema().db.file_text(ctx.file_id());
    let mut items = Vec::new();
    for conn in instance.connections().children() {
        let named = conn.as_named_port_connection()?;
        let name = named.name()?.value_text().to_string();
        let range = conn.syntax().text_range()?;
        items.push((name, text_at(&text, range)?, range));
    }

    let replacement = sorted_list_replacement(&text, open, close, items)?;
    let range = TextRange::new(open.end(), close.start());
    collector.add(ID, LABEL, range, |builder| {
        builder.replace(range, replacement);
    });

    Some(())
}

pub(crate) fn sorted_list_replacement(
    text: &str,
    open: TextRange,
    close: TextRange,
    mut items: Vec<(String, String, TextRange)>,
) -> Option<String> {
    if items.len() < 2 {
        return None;
    }

    let sorted_names = {
        let mut names = items.iter().map(|(name, _, _)| name.clone()).collect::<Vec<_>>();
        names.sort();
        names
    };
    if items.iter().map(|(name, _, _)| name).eq(sorted_names.iter()) {
        return None;
    }

    items.sort_by(|(lhs, _, _), (rhs, _, _)| lhs.cmp(rhs));

    let content = text.get(usize::from(open.end())..usize::from(close.start()))?;
    if content.contains('\n') {
        let newline = newline_style(text);
        let close_indent = line_indent(text, close.start());
        let item_indent = items
            .first()
            .map(|(_, _, range)| line_indent(text, range.start()))
            .filter(|indent| !indent.is_empty())
            .unwrap_or_else(|| format!("{close_indent}    "));
        let rendered = items
            .into_iter()
            .map(|(_, item, _)| format!("{item_indent}{}", item.trim()))
            .collect::<Vec<_>>()
            .join(&format!(",{newline}"));
        Some(format!("{newline}{rendered}{newline}{close_indent}"))
    } else {
        Some(
            items
                .into_iter()
                .map(|(_, item, _)| item.trim().to_owned())
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}
