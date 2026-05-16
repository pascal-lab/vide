use syntax::{SyntaxNodeExt, ast, ast::AstNode, has_text_range::HasTextRange};
use utils::line_index::TextSize;

use crate::completion::{
    context::{
        CompletionExpectation, ExpectationSource, ExpectedSyntax, PortListKind, TriggerChar,
        caret::CaretSnapshot,
    },
    syntax_keywords,
};

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

pub(super) fn potential_ansi_port_item_start(
    caret: &CaretSnapshot<'_>,
    trigger: Option<TriggerChar>,
) -> Option<CompletionExpectation> {
    let (replacement, prefix) = caret.replacement_and_prefix();
    if !is_in_port_or_module_header_context(caret) {
        return None;
    }

    let prev = caret.root.token_before_offset(replacement.start())?;
    let prev_text = prev.tok.raw_text();
    let is_first_port = prev_text.as_bytes() == b"(";
    let is_next_port_item =
        prev_text.as_bytes() == b"," && is_in_ansi_or_function_port_context(caret);
    if !is_first_port && !is_next_port_item {
        return None;
    }

    let syntax = if is_in_function_port_list_context(caret) {
        ExpectedSyntax::FunctionPortItem
    } else {
        ExpectedSyntax::AnsiPortItem
    };
    let is_expected = if prefix.is_empty() {
        trigger == Some(TriggerChar::Newline)
    } else {
        port_keyword_kind(syntax)
            .is_some_and(|kind| syntax_keywords::has_port_item_keyword_prefix(&prefix, kind))
    };

    if !is_expected {
        return None;
    }

    Some(CompletionExpectation { syntax, source: ExpectationSource::ParserRecovery })
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
        .and_then(|declarator| declarator.name())
        .and_then(|name| name.text_range())
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

fn is_in_port_or_module_header_context(caret: &CaretSnapshot<'_>) -> bool {
    is_in_ansi_or_function_port_context(caret)
        || caret.root.find_node_at_offset::<ast::NonAnsiPortList<'_>>(caret.offset).is_some()
        || is_in_module_header(caret)
}

fn is_in_ansi_or_function_port_context(caret: &CaretSnapshot<'_>) -> bool {
    caret.root.find_node_at_offset::<ast::AnsiPortList<'_>>(caret.offset).is_some()
        || is_in_function_port_list_context(caret)
}

fn is_in_function_port_list_context(caret: &CaretSnapshot<'_>) -> bool {
    caret.root.find_node_at_offset::<ast::FunctionPortList<'_>>(caret.offset).is_some()
}

fn is_in_module_header(caret: &CaretSnapshot<'_>) -> bool {
    let Some(module) = caret.root.find_node_at_offset::<ast::ModuleDeclaration<'_>>(caret.offset)
    else {
        return false;
    };

    module
        .header()
        .syntax()
        .text_range()
        .is_some_and(|range| range.contains(caret.offset) || range.end() == caret.offset)
}

fn port_keyword_kind(syntax: ExpectedSyntax) -> Option<PortListKind> {
    match syntax {
        ExpectedSyntax::AnsiPortItem => Some(PortListKind::Ansi),
        ExpectedSyntax::FunctionPortItem => Some(PortListKind::Function),
        _ => None,
    }
}
