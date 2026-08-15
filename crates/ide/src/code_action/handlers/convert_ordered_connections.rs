use std::ops::Range;

use base_db::source_db::SourceDb;
use hir_def::{
    container::OwnerRef,
    module::instantiation::{ParamAssign, PortConn},
};
use itertools::Itertools;
use syntax::ast;

use crate::{
    code_action::{
        CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
        leading_overridable_parameter_names, port_names,
    },
    module_resolution::resolve_hir_instantiation_target,
};

const PORTS_ID: CodeActionId = CodeActionId {
    name: "convert_ordered_ports",
    kind: CodeActionKind::RefactorRewrite,
    repair: Some(RepairKind::ConvertOrderedPorts),
};
const PORTS_LABEL: &str = "Convert ordered port connections to named connections";

const PARAMS_ID: CodeActionId = CodeActionId {
    name: "convert_ordered_params",
    kind: CodeActionKind::RefactorRewrite,
    repair: Some(RepairKind::ConvertOrderedParams),
};
const PARAMS_LABEL: &str = "Convert ordered parameter assignments to named assignments";

// Assist: convert_ordered_ports
//
// This converts ordered port connections to named port connections using the
// target module's port order.
//
// ```
// child u($0a, b);
// ```
// ->
// ```
// child u(.a(a), .b(b));
// ```
pub(super) fn convert_ordered_ports(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let sema = ctx.sema();
    let db = sema.db;
    let text = db.file_text(ctx.file_id());
    let ast_instance = ctx.find_node_at_offset::<ast::HierarchicalInstance>()?;
    let OwnerRef { value: instance_id, cont_id: module_id } =
        sema.resolve_instance(ctx.file_id().into(), ast_instance)?;
    let module = db.body_with_source_map(module_id);
    let module_body = db.body_with_source_map(module_id);
    let instantiation = module.get(module.get(instance_id).parent);
    let target_module_id = resolve_hir_instantiation_target(db, &crate::module_resolution::module_indexes(db), ctx.file_id(), instantiation)?;
    let target_module = db.body_with_source_map(target_module_id);
    let target_body = db.body_with_source_map(target_module_id);
    let port_names = port_names(&target_module, &target_body);

    let replacements = module
        .get(instance_id)
        .connections
        .iter()
        .enumerate()
        .filter_map(|(idx, conn_id)| {
            let PortConn::Ordered(expr_id) = module.get(*conn_id) else {
                return None;
            };
            let name = port_names.get(idx)?;
            let expr = module_body.source_range(ctx.sema().db, *expr_id)?;
            let range = module.source_range(ctx.sema().db, *conn_id)?;
            Some((range, format!(".{name}({})", text.get(Range::from(expr))?)))
        })
        .collect_vec();

    if replacements.is_empty() {
        return None;
    }

    collector.add(PORTS_ID, PORTS_LABEL, ctx.range(), |builder| {
        for (range, text) in replacements {
            builder.replace(range, text);
        }
    });

    Some(())
}

// Assist: convert_ordered_params
//
// This converts ordered parameter assignments to named parameter assignments
// using the target module's parameter order.
//
// ```
// child #($01, 2) u();
// ```
// ->
// ```
// child #(.A(1), .B(2)) u();
// ```
pub(super) fn convert_ordered_params(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let sema = ctx.sema();
    let db = sema.db;
    let text = db.file_text(ctx.file_id());
    let ast_instantiation = ctx.find_node_at_offset::<ast::HierarchyInstantiation>()?;
    let OwnerRef { value: instantiation_id, cont_id: module_id } =
        sema.resolve_instantiation(ctx.file_id().into(), ast_instantiation)?;
    let module = db.body_with_source_map(module_id);
    let module_body = db.body_with_source_map(module_id);
    let instantiation = module.get(instantiation_id);
    let target_module_id = resolve_hir_instantiation_target(db, &crate::module_resolution::module_indexes(db), ctx.file_id(), instantiation)?;
    let target_body = db.body_with_source_map(target_module_id);
    let param_names = leading_overridable_parameter_names(&target_body);

    let replacements = instantiation
        .param_assigns
        .iter()
        .enumerate()
        .filter_map(|(idx, assign_id)| {
            let ParamAssign::Ordered(expr_id) = module.get(*assign_id) else {
                return None;
            };
            let name = param_names.get(idx)?;
            let expr = module_body.source_range(ctx.sema().db, *expr_id)?;
            let range = module.source_range(ctx.sema().db, *assign_id)?;
            Some((range, format!(".{name}({})", text.get(Range::from(expr))?)))
        })
        .collect_vec();

    if replacements.is_empty() {
        return None;
    }

    collector.add(PARAMS_ID, PARAMS_LABEL, ctx.range(), |builder| {
        for (range, text) in replacements {
            builder.replace(range, text);
        }
    });

    Some(())
}
