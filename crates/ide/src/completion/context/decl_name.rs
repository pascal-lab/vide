use syntax::{
    SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};

use crate::completion::context::caret::CaretSnapshot;

pub(super) fn is_in_decl_name(caret: &CaretSnapshot<'_>, parser_expects_decl_name: bool) -> bool {
    if is_in_existing_declarator_name(caret) {
        return true;
    }

    parser_expects_decl_name && !current_prefix_at_offset(caret) && is_in_declaration_context(caret)
}

fn current_prefix_at_offset(caret: &CaretSnapshot<'_>) -> bool {
    let (replacement, prefix) = caret.replacement_and_prefix();
    !prefix.is_empty()
        && replacement.end() == caret.offset
        && caret
            .root
            .token_before_offset(caret.offset)
            .and_then(|t| t.text_range())
            .is_some_and(|range| range == replacement)
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
