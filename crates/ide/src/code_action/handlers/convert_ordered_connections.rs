use std::ops::Range;

use hir::{
    base_db::source_db::SourceDb,
    container::InModule,
    db::HirDb,
    hir_def::module::instantiation::{ParamAssign, PortConn},
    source_map::IsSrc,
};
use itertools::Itertools;
use syntax::ast;
use utils::get::{Get, GetRef};

use crate::{
    code_action::{
        CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
        leading_parameter_names, port_names,
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
    let InModule { value: instance_id, module_id } =
        sema.resolve_instance(ctx.file_id().into(), ast_instance)?;
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let instantiation = module.get(module.get(instance_id).parent);
    let target_module_id = resolve_hir_instantiation_target(db, ctx.file_id(), instantiation)?;
    let port_names = port_names(&db.module(target_module_id));

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
            let expr = module_src_map.get(*expr_id)?.range();
            let range = module_src_map.get(*conn_id)?.range();
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
    let InModule { value: instantiation_id, module_id } =
        sema.resolve_instantiation(ctx.file_id().into(), ast_instantiation)?;
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let instantiation = module.get(instantiation_id);
    let target_module_id = resolve_hir_instantiation_target(db, ctx.file_id(), instantiation)?;
    let target_module = db.module(target_module_id);
    let param_names = leading_parameter_names(&target_module);

    let replacements = instantiation
        .param_assigns
        .iter()
        .enumerate()
        .filter_map(|(idx, assign_id)| {
            let ParamAssign::Ordered(expr_id) = module.get(*assign_id) else {
                return None;
            };
            let name = param_names.get(idx)?;
            let expr = module_src_map.get(*expr_id)?.range();
            let range = module_src_map.get(*assign_id)?.range();
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
