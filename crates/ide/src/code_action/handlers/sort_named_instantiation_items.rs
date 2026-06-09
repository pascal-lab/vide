use std::ops::Range;

use hir::{
    base_db::source_db::SourceDb,
    container::InModule,
    db::HirDb,
    hir_def::module::instantiation::{ParamAssign, PortConn},
    source_map::IsSrc,
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRangeIn,
};
use utils::{
    get::{Get, GetRef},
    text_edit::TextRange,
};

use crate::{
    code_action::{
        CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, all_parameter_names,
        line_indent, port_names,
    },
    module_resolution::resolve_hir_instantiation_target,
};

const SORT_NAMED_PARAMETER_ASSIGNMENTS_ID: CodeActionId = CodeActionId {
    name: "sort_named_parameter_assignments",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const SORT_NAMED_PARAMETER_ASSIGNMENTS_LABEL: &str = "Sort named parameter assignments";

// Assist: sort_named_parameter_assignments
//
// This sorts named parameter assignments to match the target module's parameter
// declaration order.
//
// ```
// child #(.B(2), $0.A(1)) u();
// ```
// ->
// ```
// child #(.A(1), .B(2)) u();
// ```
pub(super) fn sort_named_parameter_assignments(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let ast_instantiation = ctx.find_node_at_offset::<ast::HierarchyInstantiation>()?;
    let params = ast_instantiation.parameters()?;
    let open = params.open_paren()?.text_range_in(params.syntax())?;
    let close = params.close_paren()?.text_range_in(params.syntax())?;

    let sema = ctx.sema();
    let db = sema.db;
    let InModule { value: instantiation_id, module_id } =
        sema.resolve_instantiation(ctx.file_id().into(), ast_instantiation)?;
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let instantiation = module.get(instantiation_id);
    let target_module_id = resolve_hir_instantiation_target(db, ctx.file_id(), instantiation)?;
    let parameter_order = all_parameter_names(&db.module(target_module_id));
    let parameter_order_map: FxHashMap<_, _> =
        parameter_order.iter().enumerate().map(|(index, name)| (name.as_ref(), index)).collect();

    let text = sema.db.file_text(ctx.file_id());
    let mut items = Vec::new();
    for assign_id in instantiation.param_assigns.iter() {
        let ParamAssign::Named(Some(name), _) = module.get(*assign_id) else {
            return None;
        };
        let order = *parameter_order_map.get(name.as_str())?;
        let range = module_src_map.get(*assign_id)?.expanded_range();
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

// Assist: sort_named_port_connections
//
// This sorts named port connections to match the target module's port
// declaration order.
//
// ```
// child u(.b(b), $0.a(a));
// ```
// ->
// ```
// child u(.a(a), .b(b));
// ```
pub(super) fn sort_named_port_connections(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let ast_instance = ctx.find_node_at_offset::<ast::HierarchicalInstance>()?;
    let open = ast_instance.open_paren()?.text_range_in(ast_instance.syntax())?;
    let close = ast_instance.close_paren()?.text_range_in(ast_instance.syntax())?;

    let sema = ctx.sema();
    let db = sema.db;
    let InModule { value: instance_id, module_id } =
        sema.resolve_instance(ctx.file_id().into(), ast_instance)?;
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let instance = module.get(instance_id);
    let instantiation = module.get(instance.parent);
    let target_module_id = resolve_hir_instantiation_target(db, ctx.file_id(), instantiation)?;
    let port_order = port_names(&db.module(target_module_id));
    let port_order_map: FxHashMap<_, _> =
        port_order.iter().enumerate().map(|(index, name)| (name.as_ref(), index)).collect();

    let text = sema.db.file_text(ctx.file_id());
    let mut items = Vec::new();
    for conn_id in instance.connections.iter() {
        let PortConn::Named(Some(name), _) = module.get(*conn_id) else {
            return None;
        };
        let order = *port_order_map.get(name.as_str())?;
        let range = module_src_map.get(*conn_id)?.expanded_range();
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
