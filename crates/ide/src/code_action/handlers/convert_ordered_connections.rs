use base_db::source_db::SourceDb;
use hir::db::HirDb;
use itertools::Itertools;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
    leading_parameter_names, port_names,
};

const PORTS_ID: CodeActionId =
    CodeActionId { name: "convert_ordered_ports", kind: CodeActionKind::Generate };
const PORTS_LABEL: &str = "Convert ordered port connections to named connections";

const PARAMS_ID: CodeActionId =
    CodeActionId { name: "convert_ordered_params", kind: CodeActionKind::Generate };
const PARAMS_LABEL: &str = "Convert ordered parameter assignments to named assignments";

pub(super) fn convert_ordered_ports(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    if !ctx.diagnostics.allows_repair(RepairKind::ConvertOrderedPorts) {
        return None;
    }

    let sema = ctx.sema;
    let db = sema.db;
    let ast_instance = ctx.find_node_at_offset::<ast::HierarchicalInstance>()?;
    let instantiation = ast::HierarchyInstantiation::cast(ast_instance.syntax().parent()?)?;
    let target_module_id = sema.nameres_instantiation(instantiation)?;
    let target_module = db.module(target_module_id);
    let port_names = port_names(&target_module);

    let replacements = ast_instance
        .connections()
        .children()
        .enumerate()
        .filter_map(|(idx, conn)| {
            let ordered = conn.as_ordered_port_connection()?;
            let name = port_names.get(idx)?;
            let expr = ordered.expr().syntax().text_range()?;
            let range = ordered.syntax().text_range()?;
            Some((range, format!(".{name}({})", text_at(ctx, expr)?)))
        })
        .collect_vec();

    if replacements.is_empty() {
        return None;
    }

    collector.add(PORTS_ID, PORTS_LABEL, ctx.range, |builder| {
        for (range, text) in replacements {
            builder.replace(range, text);
        }
    });

    Some(())
}

pub(super) fn convert_ordered_params(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    if !ctx.diagnostics.allows_repair(RepairKind::ConvertOrderedParams) {
        return None;
    }

    let sema = ctx.sema;
    let db = sema.db;
    let ast_instantiation = ctx.find_node_at_offset::<ast::HierarchyInstantiation>()?;
    let target_module_id = sema.nameres_instantiation(ast_instantiation)?;
    let target_module = db.module(target_module_id);
    let param_names = leading_parameter_names(&target_module);

    let replacements = ast_instantiation
        .parameters()?
        .parameters()
        .children()
        .enumerate()
        .filter_map(|(idx, assign)| {
            let ordered = assign.as_ordered_param_assignment()?;
            let name = param_names.get(idx)?;
            let expr = ordered.expr().syntax().text_range()?;
            let range = ordered.syntax().text_range()?;
            Some((range, format!(".{name}({})", text_at(ctx, expr)?)))
        })
        .collect_vec();

    if replacements.is_empty() {
        return None;
    }

    collector.add(PARAMS_ID, PARAMS_LABEL, ctx.range, |builder| {
        for (range, text) in replacements {
            builder.replace(range, text);
        }
    });

    Some(())
}

fn text_at(ctx: &CodeActionCtx, range: utils::text_edit::TextRange) -> Option<String> {
    let text = ctx.sema.db.file_text(ctx.file_id);
    Some(text[std::ops::Range::<usize>::from(range)].to_owned())
}
