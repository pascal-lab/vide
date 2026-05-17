use syntax::{
    SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::line_index::TextSize;

use crate::completion::context::caret::CaretSnapshot;

pub(super) fn is_in_decl_name(
    caret: &CaretSnapshot<'_>,
    expected_decl_name_offsets: Option<&[TextSize]>,
) -> bool {
    if is_in_existing_declarator_name(caret) {
        return true;
    }

    if let Some(offsets) = expected_decl_name_offsets
        && expected_decl_name_hit(caret, offsets)
        && is_in_declaration_context(caret)
    {
        return true;
    }

    false
}

fn expected_decl_name_hit(caret: &CaretSnapshot<'_>, offsets: &[TextSize]) -> bool {
    let (replacement, prefix) = caret.replacement_and_prefix();
    let current_prefix_at_offset = !prefix.is_empty()
        && replacement.end() == caret.offset
        && caret
            .root
            .token_before_offset(caret.offset)
            .and_then(|t| t.text_range())
            .is_some_and(|range| range == replacement);

    let candidates = [
        Some(caret.offset),
        caret
            .root
            .token_after_or_at_offset(caret.offset)
            .and_then(|t| t.text_range())
            .map(|r| r.start()),
        caret.root.token_before_offset(caret.offset).and_then(|t| t.text_range()).map(|r| r.end()),
    ];

    candidates.into_iter().flatten().any(|off| {
        !(current_prefix_at_offset && off == caret.offset) && offsets.binary_search(&off).is_ok()
    })
}

fn is_in_existing_declarator_name(caret: &CaretSnapshot<'_>) -> bool {
    caret
        .root
        .find_node_at_offset::<ast::Declarator<'_>>(caret.offset)
        .and_then(|declarator| {
            declarator.name().and_then(|name| name.text_range_in(declarator.syntax()))
        })
        .is_some_and(|range| range.contains(caret.offset) || range.end() == caret.offset)
}

fn is_in_declaration_context(caret: &CaretSnapshot<'_>) -> bool {
    let offset = caret.offset;
    caret.root.find_node_at_offset::<ast::AnsiPortList<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::NonAnsiPortList<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::FunctionPortList<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::DataDeclaration<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::NetDeclaration<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::LocalVariableDeclaration<'_>>(offset).is_some()
        || caret
            .root
            .find_node_at_offset::<ast::ParameterDeclarationStatement<'_>>(offset)
            .is_some()
        || caret.root.find_node_at_offset::<ast::GenvarDeclaration<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::TypedefDeclaration<'_>>(offset).is_some()
}
