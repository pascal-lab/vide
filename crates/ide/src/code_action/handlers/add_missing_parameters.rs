use hir::{
    base_db::source_db::SourceDb, container::InModule, db::HirDb,
    hir_def::module::instantiation::ParamAssign,
};
use rustc_hash::FxHashSet;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::get::GetRef;

use crate::{
    code_action::{
        CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
        all_parameter_names, apply_missing_list_edit, leading_parameter_names,
        missing_member_entry_text,
    },
    module_resolution::resolve_instantiation_target,
};

const ID: CodeActionId = CodeActionId {
    name: "add_missing_parameters",
    kind: CodeActionKind::Generate,
    repair: Some(RepairKind::MissingParameter),
};
const LABEL: &str = "Fill parameters";

pub(super) fn add_missing_parameters(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let sema = ctx.sema();
    let db = sema.db;
    let file_id = ctx.file_id().into();

    let ast_instantiation = ctx.find_node_at_offset::<ast::HierarchyInstantiation>()?;
    let InModule { value: instantiation_id, module_id } =
        sema.resolve_instantiation(file_id, ast_instantiation)?;
    let module = db.module(module_id);
    let instantiation = module.get(instantiation_id);

    let params_node = ast_instantiation.parameters()?;
    let open_paren = params_node.open_paren()?.text_range_in(params_node.syntax())?;
    let close_paren = params_node.close_paren()?.text_range_in(params_node.syntax())?;

    let target_module_id =
        resolve_instantiation_target(db, ctx.file_id(), ast_instantiation).unique()?;
    let target_module = db.module(target_module_id);

    let is_ordered = instantiation
        .param_assigns
        .first()
        .map(|id| matches!(module.get(*id), ParamAssign::Ordered(_)))
        .unwrap_or_default();

    let names: Vec<_> = if is_ordered {
        leading_parameter_names(&target_module)
            .into_iter()
            .skip(instantiation.param_assigns.len())
            .collect()
    } else {
        let mut assigned_names = FxHashSet::default();
        for param_id in instantiation.param_assigns.iter() {
            match module.get(*param_id) {
                ParamAssign::Named(Some(name), _) => {
                    assigned_names.insert(name.clone());
                }
                ParamAssign::Ordered(_) => return None,
                _ => {}
            }
        }

        all_parameter_names(&target_module)
            .into_iter()
            .filter(|name| !assigned_names.contains(name))
            .collect()
    };

    if names.is_empty() {
        return None;
    }

    collector.add(ID, LABEL, ctx.range(), |builder| {
        let entries = names
            .into_iter()
            .map(|name| missing_member_entry_text(sema, module_id, name, is_ordered, "0"))
            .collect();

        let text = sema.db.file_text(ctx.file_id());
        let item_ranges = params_node.parameters().children().filter_map(|assign| {
            let range = assign.syntax().text_range()?;
            (!range.is_empty()).then_some(range)
        });
        apply_missing_list_edit(builder, &text, open_paren, close_paren, item_ranges, entries);
    });

    Some(())
}
