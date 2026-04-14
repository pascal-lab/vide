use syntax::{SyntaxNodeExt, ast, has_text_range::HasTextRange};
use utils::line_index::TextSize;

use super::caret::CaretSnapshot;

pub(super) fn is_in_decl_name(
    caret: &CaretSnapshot<'_>,
    expected_identifier_offsets: Option<&[TextSize]>,
) -> bool {
    if is_in_existing_declarator_name(caret) {
        return true;
    }

    if let Some(offsets) = expected_identifier_offsets
        && expected_identifier_hit(caret, offsets)
        && is_in_port_list(caret)
    {
        return true;
    }

    false
}

fn expected_identifier_hit(caret: &CaretSnapshot<'_>, offsets: &[TextSize]) -> bool {
    [
        Some(caret.offset),
        caret
            .root
            .token_after_or_at_offset(caret.offset)
            .and_then(|t| t.text_range())
            .map(|r| r.start()),
        caret.root.token_before_offset(caret.offset).and_then(|t| t.text_range()).map(|r| r.end()),
    ]
    .into_iter()
    .flatten()
    .any(|off| offsets.binary_search(&off).is_ok())
}

fn is_in_existing_declarator_name(caret: &CaretSnapshot<'_>) -> bool {
    caret
        .root
        .find_node_at_offset::<ast::Declarator<'_>>(caret.offset)
        .and_then(|declarator| declarator.name())
        .and_then(|name| name.text_range())
        .is_some_and(|range| range.contains(caret.offset) || range.end() == caret.offset)
}

fn is_in_port_list(caret: &CaretSnapshot<'_>) -> bool {
    let offset = caret.offset;
    caret.root.find_node_at_offset::<ast::AnsiPortList<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::NonAnsiPortList<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::FunctionPortList<'_>>(offset).is_some()
}
