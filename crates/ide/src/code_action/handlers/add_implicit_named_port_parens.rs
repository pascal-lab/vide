use syntax::{ast, has_text_range::HasTextRange};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
};

const ID: CodeActionId =
    CodeActionId { name: "add_implicit_named_port_parens", kind: CodeActionKind::Generate };
const LABEL: &str = "Add explicit empty port connection";

pub(super) fn add_implicit_named_port_parens(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    if !ctx.diagnostics.allows_repair(RepairKind::AddImplicitNamedPortParens) {
        return None;
    }

    let conn = ctx.find_node_at_offset::<ast::NamedPortConnection>()?;
    if conn.open_paren().is_some() {
        return None;
    }

    let insert_offset = conn.name()?.text_range()?.end();
    collector.add(ID, LABEL, ctx.range, |builder| {
        builder.insert(insert_offset, "()".to_owned());
    });

    Some(())
}
