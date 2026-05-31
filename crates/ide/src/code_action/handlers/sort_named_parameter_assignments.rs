use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::text_edit::TextRange;

use super::sort_named_port_connections::sorted_list_replacement;
use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, text_at,
};

const ID: CodeActionId = CodeActionId {
    name: "sort_named_parameter_assignments",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const LABEL: &str = "Sort named parameter assignments";

pub(super) fn sort_named_parameter_assignments(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let instantiation = ctx.find_node_at_offset::<ast::HierarchyInstantiation>()?;
    let params = instantiation.parameters()?;
    let open = params.open_paren()?.text_range_in(params.syntax())?;
    let close = params.close_paren()?.text_range_in(params.syntax())?;

    let text = ctx.sema().db.file_text(ctx.file_id());
    let mut items = Vec::new();
    for assign in params.parameters().children() {
        let named = assign.as_named_param_assignment()?;
        let name = named.name()?.value_text().to_string();
        let range = assign.syntax().text_range()?;
        items.push((name, text_at(&text, range)?, range));
    }

    let replacement = sorted_list_replacement(&text, open, close, items)?;
    let range = TextRange::new(open.end(), close.start());
    collector.add(ID, LABEL, range, |builder| {
        builder.replace(range, replacement);
    });

    Some(())
}
