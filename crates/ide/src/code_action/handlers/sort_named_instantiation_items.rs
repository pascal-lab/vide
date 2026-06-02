use std::ops::Range;

use hir::{base_db::source_db::SourceDb, db::HirDb};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use smol_str::ToSmolStr;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::text_edit::TextRange;

use crate::{
    code_action::{
        CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, all_parameter_names,
        line_indent, port_names,
    },
    module_resolution::resolve_instantiation_target,
};

const SORT_NAMED_PARAMETER_ASSIGNMENTS_ID: CodeActionId = CodeActionId {
    name: "sort_named_parameter_assignments",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const SORT_NAMED_PARAMETER_ASSIGNMENTS_LABEL: &str = "Sort named parameter assignments";

pub(super) fn sort_named_parameter_assignments(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let instantiation = ctx.find_node_at_offset::<ast::HierarchyInstantiation>()?;
    let params = instantiation.parameters()?;
    let open = params.open_paren()?.text_range_in(params.syntax())?;
    let close = params.close_paren()?.text_range_in(params.syntax())?;

    let db = ctx.sema().db;
    let target_module_id =
        resolve_instantiation_target(db, ctx.file_id(), instantiation).unique()?;
    let parameter_order = all_parameter_names(&db.module(target_module_id));
    let parameter_order_map: FxHashMap<_, _> =
        parameter_order.iter().enumerate().map(|(index, name)| (name.as_ref(), index)).collect();

    let text = ctx.sema().db.file_text(ctx.file_id());
    let mut items = Vec::new();
    for assign in params.parameters().children() {
        let named = assign.as_named_param_assignment()?;
        let name = named.name()?.value_text().to_smolstr();
        let order = *parameter_order_map.get(name.as_str())?;
        let range = assign.syntax().text_range()?;
        items.push((order, text.get(Range::from(range))?, range));
    }

    add_sorted_list_action(
        collector,
        SORT_NAMED_PARAMETER_ASSIGNMENTS_ID,
        SORT_NAMED_PARAMETER_ASSIGNMENTS_LABEL,
        &text,
        open,
        close,
        items,
    )
}

const SORT_NAMED_PORT_CONNECTIONS_ID: CodeActionId = CodeActionId {
    name: "sort_named_port_connections",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const SORT_NAMED_PORT_CONNECTIONS_LABEL: &str = "Sort named port connections";

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
    let port_order_map: FxHashMap<_, _> =
        port_order.iter().enumerate().map(|(index, name)| (name.as_ref(), index)).collect();

    let text = ctx.sema().db.file_text(ctx.file_id());
    let mut items = Vec::new();
    for conn in instance.connections().children() {
        let named = conn.as_named_port_connection()?;
        let name = named.name()?.value_text().to_smolstr();
        let order = *port_order_map.get(name.as_str())?;
        let range = conn.syntax().text_range()?;
        items.push((order, text.get(Range::from(range))?, range));
    }

    add_sorted_list_action(
        collector,
        SORT_NAMED_PORT_CONNECTIONS_ID,
        SORT_NAMED_PORT_CONNECTIONS_LABEL,
        &text,
        open,
        close,
        items,
    )
}

fn add_sorted_list_action(
    collector: &mut CodeActionCollector,
    id: CodeActionId,
    label: &'static str,
    text: &str,
    open: TextRange,
    close: TextRange,
    mut items: Vec<(usize, &str, TextRange)>,
) -> Option<()> {
    if items.len() < 2 || items.windows(2).all(|items| items[0].0 < items[1].0) {
        return None;
    }

    items.sort_by_key(|(order, _, _)| *order);

    let content = text.get(open.end().into()..close.start().into())?;

    let range = TextRange::new(open.end(), close.start());
    collector.add(id, label, range, |builder| {
        let replacement = if content.contains('\n') {
            let close_indent = line_indent(text, close.start());
            let item_indent = items
                .first()
                .map(|(_, _, range)| line_indent(text, range.start()))
                .filter(|indent| !indent.is_empty())
                .unwrap_or_else(|| format!("{close_indent}    "));
            let rendered =
                items.into_iter().map(|(_, item, _)| format!("{item_indent}{item}")).join(",\n");

            format!("\n{rendered}\n{close_indent}")
        } else {
            items.into_iter().map(|(_, name, _)| name).join(", ")
        };
        builder.replace(range, replacement);
    });
    Some(())
}
