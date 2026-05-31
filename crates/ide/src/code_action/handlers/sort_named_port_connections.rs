use hir::{base_db::source_db::SourceDb, db::HirDb};
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::text_edit::TextRange;

use crate::{
    code_action::{
        CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent,
        newline_style, port_names, text_at,
    },
    module_resolution::resolve_instantiation_target,
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
    let instantiation = ast::HierarchyInstantiation::cast(instance.syntax().parent()?)?;
    let open = instance.open_paren()?.text_range_in(instance.syntax())?;
    let close = instance.close_paren()?.text_range_in(instance.syntax())?;

    let db = ctx.sema().db;
    let target_module_id =
        resolve_instantiation_target(db, ctx.file_id(), instantiation).unique()?;
    let target_module = db.module(target_module_id);
    let port_order = port_names(&target_module);

    let text = ctx.sema().db.file_text(ctx.file_id());
    let mut items = Vec::new();
    for conn in instance.connections().children() {
        let named = conn.as_named_port_connection()?;
        let name = named.name()?.value_text().to_string();
        let order = port_order.iter().position(|port| port.as_str() == name)?;
        let range = conn.syntax().text_range()?;
        items.push((order, text_at(&text, range)?, range));
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
    mut items: Vec<(usize, String, TextRange)>,
) -> Option<String> {
    if items.len() < 2 {
        return None;
    }

    if items.windows(2).all(|items| items[0].0 < items[1].0) {
        return None;
    }

    items.sort_by_key(|(order, _, _)| *order);

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
