use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRangeIn,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
};

const ID: CodeActionId = CodeActionId {
    name: "add_implicit_named_port_parens",
    kind: CodeActionKind::Generate,
    repair: Some(RepairKind::AddImplicitNamedPortParens),
};
const LABEL: &str = "Add explicit empty port connection";

pub(super) fn add_implicit_named_port_parens(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let conn = ctx.find_node_at_offset::<ast::NamedPortConnection>()?;
    if conn.open_paren().is_some() {
        return None;
    }

    let insert_offset = conn.name()?.text_range_in(conn.syntax())?.end();
    collector.add(ID, LABEL, ctx.range(), |builder| {
        builder.insert(insert_offset, "()".to_owned());
    });

    Some(())
}
