use either::Either;
use syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt};
use utils::text_edit::TextSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionContextKind {
    /// In an expression context (assignments, conditions, etc.)
    Expression,

    /// In a type reference position (variable declarations, port types, etc.)
    TypeReference,

    /// In a module/interface instantiation
    Instantiation,

    /// In a port connection list
    PortConnection,

    /// In a parameter/argument list
    ParameterList,

    /// At module/class/interface member level
    MemberDeclaration,

    /// Unknown or generic context
    Unknown,
}

pub fn analyze_context(root: SyntaxNode, position: TextSize) -> CompletionContextKind {
    let token_or_node = root.token_or_node_at_offset(position);

    let mut node = match token_or_node {
        Either::Left(tok_at_offset) => match tok_at_offset.left_biased() {
            Some(tok) => tok.parent,
            None => return CompletionContextKind::Unknown,
        },
        Either::Right(node) => node,
    };

    loop {
        let kind = node.kind();

        if is_port_connection_context(kind) {
            return CompletionContextKind::PortConnection;
        }

        if kind == SyntaxKind::HIERARCHICAL_INSTANCE {
            return CompletionContextKind::PortConnection;
        }

        if is_expression_context(kind) {
            return CompletionContextKind::Expression;
        }

        if is_type_reference_context(kind) {
            return CompletionContextKind::TypeReference;
        }

        if is_instantiation_context(kind) {
            return CompletionContextKind::Instantiation;
        }

        if is_parameter_list_context(kind) {
            return CompletionContextKind::ParameterList;
        }

        if is_member_declaration_context(kind) {
            return CompletionContextKind::MemberDeclaration;
        }

        let parent = node.parent();
        if let Some(p) = parent {
            node = p;
        } else {
            break;
        }
    }

    CompletionContextKind::Unknown
}

fn is_expression_context(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ASSIGNMENT_EXPRESSION
            | SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION
            | SyntaxKind::BINARY_AND_EXPRESSION
            | SyntaxKind::BINARY_OR_EXPRESSION
            | SyntaxKind::BINARY_XOR_EXPRESSION
            | SyntaxKind::BINARY_XNOR_EXPRESSION
            | SyntaxKind::CONDITIONAL_EXPRESSION
            | SyntaxKind::INVOCATION_EXPRESSION
            | SyntaxKind::PARENTHESIZED_EXPRESSION
            | SyntaxKind::EXPRESSION_STATEMENT
            | SyntaxKind::RETURN_STATEMENT
            | SyntaxKind::CONDITIONAL_STATEMENT
            | SyntaxKind::CASE_STATEMENT
            | SyntaxKind::FOR_LOOP_STATEMENT
            | SyntaxKind::FOREACH_LOOP_STATEMENT
            | SyntaxKind::DO_WHILE_STATEMENT
            | SyntaxKind::FOREVER_STATEMENT
            | SyntaxKind::LOOP_STATEMENT
            | SyntaxKind::ALWAYS_BLOCK
            | SyntaxKind::ALWAYS_COMB_BLOCK
            | SyntaxKind::ALWAYS_FF_BLOCK
            | SyntaxKind::ALWAYS_LATCH_BLOCK
            | SyntaxKind::INITIAL_BLOCK
            | SyntaxKind::FINAL_BLOCK
            | SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT
            | SyntaxKind::PARALLEL_BLOCK_STATEMENT
    )
}

fn is_type_reference_context(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::DATA_DECLARATION
            | SyntaxKind::LOCAL_VARIABLE_DECLARATION
            | SyntaxKind::FOR_VARIABLE_DECLARATION
            | SyntaxKind::FUNCTION_PORT
            | SyntaxKind::PARAMETER_DECLARATION_STATEMENT
    )
}

fn is_instantiation_context(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::HIERARCHICAL_INSTANCE | SyntaxKind::HIERARCHY_INSTANTIATION)
}

fn is_port_connection_context(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::NAMED_PORT_CONNECTION | SyntaxKind::ORDERED_PORT_CONNECTION)
}

fn is_parameter_list_context(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::PARAMETER_VALUE_ASSIGNMENT | SyntaxKind::ARGUMENT_LIST)
}

fn is_member_declaration_context(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::MODULE_DECLARATION
            | SyntaxKind::CLASS_DECLARATION
            | SyntaxKind::INTERFACE_DECLARATION
            | SyntaxKind::PACKAGE_DECLARATION
    )
}
