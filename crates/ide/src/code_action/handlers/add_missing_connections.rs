use base_db::source_db::SourceDb;
use hir::{container::InModule, db::HirDb, hir_def::module::instantiation::PortConn};
use rustc_hash::FxHashSet;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::get::GetRef;

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
    apply_missing_list_edit, missing_member_entry_text, port_names, remaining_ordered_port_names,
};

const ID: CodeActionId = CodeActionId {
    name: "add_missing_connections",
    kind: CodeActionKind::Generate,
    repair: Some(RepairKind::MissingConnection),
};
const LABEL: &str = "Fill connections";

pub(super) fn add_missing_connections(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    if !ctx.diagnostics.allows_repair(RepairKind::MissingConnection) {
        return None;
    }

    let sema = ctx.sema;
    let db = sema.db;
    let file_id = ctx.file_id.into();

    let ast_instance = ctx.find_node_at_offset::<ast::HierarchicalInstance>()?;
    let InModule { value: instance_id, module_id } =
        sema.resolve_instance(file_id, ast_instance)?;
    let module = db.module(module_id);
    let instance = module.get(instance_id);
    let open_paren = ast_instance.open_paren()?.text_range_in(ast_instance.syntax())?;
    let close_paren = ast_instance.close_paren()?.text_range_in(ast_instance.syntax())?;

    let instantiation = ast::HierarchyInstantiation::cast(ast_instance.syntax().parent()?)?;
    let target_module_id = sema.nameres_instantiation(instantiation)?;
    let target_module = db.module(target_module_id);

    let is_ordered = instance
        .connections
        .first()
        .map(|id| matches!(module.get(*id), PortConn::Ordered(_) | PortConn::Empty))
        .unwrap_or_default();

    let names: Vec<_> = if is_ordered {
        remaining_ordered_port_names(&target_module, instance.connections.len())
    } else {
        let mut connected_names = FxHashSet::default();
        for conn_id in instance.connections.iter() {
            match module.get(*conn_id) {
                PortConn::Named(Some(name), _) => {
                    connected_names.insert(name.clone());
                }
                PortConn::Ordered(_) => return None,
                _ => {}
            }
        }

        port_names(&target_module)
            .into_iter()
            .filter(|name| !connected_names.contains(name))
            .collect()
    };

    if names.is_empty() {
        return None;
    }

    collector.add(ID, LABEL, ctx.range, |builder| {
        let entries = names
            .into_iter()
            .map(|name| missing_member_entry_text(sema, module_id, name, is_ordered, "'0"))
            .collect();

        let text = sema.db.file_text(ctx.file_id);
        let item_ranges = ast_instance.connections().children().filter_map(|conn| {
            let range = conn.syntax().text_range()?;
            (!range.is_empty()).then_some(range)
        });
        apply_missing_list_edit(builder, &text, open_paren, close_paren, item_ranges, entries);
    });

    Some(())
}
