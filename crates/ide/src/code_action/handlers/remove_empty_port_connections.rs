use itertools::Itertools;
use syntax::{
    SyntaxElement,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextRange;

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
};

const ID: CodeActionId = CodeActionId {
    name: "remove_empty_port_connections",
    kind: CodeActionKind::Generate,
    repair: Some(RepairKind::RemoveEmptyPortConnections),
};
const LABEL: &str = "Remove empty port connections";

pub(super) fn remove_empty_port_connections(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    if !ctx.allows_repair(RepairKind::RemoveEmptyPortConnections) {
        return None;
    }

    let ast_instance = ctx.find_node_at_offset::<ast::HierarchicalInstance>()?;
    let comma_ranges = empty_port_connection_comma_ranges(ast_instance.connections());
    if comma_ranges.is_empty() {
        return None;
    }

    collector.add(ID, LABEL, ctx.range(), |builder| {
        for range in comma_ranges {
            builder.delete(range);
        }
    });

    Some(())
}

fn empty_port_connection_comma_ranges<'a>(
    connections: ast::SeparatedList<'a, ast::PortConnection<'a>>,
) -> Vec<TextRange> {
    let children = connections.syntax().children().collect_vec();
    let mut ranges = Vec::new();

    for (idx, elem) in children.iter().enumerate() {
        let Some(node) = elem.as_node() else {
            continue;
        };
        if !matches!(
            ast::PortConnection::cast(node),
            Some(ast::PortConnection::EmptyPortConnection(_))
        ) {
            continue;
        }

        let prev_comma = idx.checked_sub(1).and_then(|idx| children.get(idx)).and_then(comma_range);
        let next_comma = children.get(idx + 1).and_then(comma_range);

        let delete_range = match (prev_comma, next_comma) {
            (Some(prev), Some(next)) => Some(TextRange::new(prev.end(), next.end())),
            (Some(prev), None) => Some(prev),
            (None, Some(next)) => Some(next),
            (None, None) => None,
        };
        if let Some(delete_range) = delete_range {
            ranges.push(delete_range);
        }
    }

    ranges.sort_unstable_by_key(|range| range.start());
    ranges.dedup();
    ranges
}

fn comma_range(elem: &SyntaxElement<'_>) -> Option<TextRange> {
    let token = elem.as_tok_with_parent()?;
    (token.value_text().as_bytes() == b",").then(|| elem.text_range()).flatten()
}
