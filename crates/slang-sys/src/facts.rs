use crate::{
    diagnostic::SyntaxKeywordContext,
    syntax::{SyntaxKind, ffi},
    token::TokenKind,
};

pub struct SemanticFacts;
pub struct SyntaxFacts;

impl SyntaxFacts {
    pub fn is_possible_statement(kind: TokenKind) -> bool {
        ffi::is_possible_statement(kind.as_raw())
    }

    pub fn is_possible_expression(kind: TokenKind) -> bool {
        ffi::is_possible_expression(kind.as_raw())
    }

    pub fn is_possible_data_type(kind: TokenKind) -> bool {
        ffi::is_possible_data_type(kind.as_raw())
    }

    pub fn is_possible_argument(kind: TokenKind) -> bool {
        ffi::is_possible_argument(kind.as_raw())
    }

    pub fn is_possible_param_assignment(kind: TokenKind) -> bool {
        ffi::is_possible_param_assignment(kind.as_raw())
    }

    pub fn is_possible_port_connection(kind: TokenKind) -> bool {
        ffi::is_possible_port_connection(kind.as_raw())
    }

    pub fn is_possible_ansi_port(kind: TokenKind) -> bool {
        ffi::is_possible_ansi_port(kind.as_raw())
    }

    pub fn is_possible_non_ansi_port(kind: TokenKind) -> bool {
        ffi::is_possible_non_ansi_port(kind.as_raw())
    }

    pub fn is_possible_function_port(kind: TokenKind) -> bool {
        ffi::is_possible_function_port(kind.as_raw())
    }

    pub fn is_possible_parameter(kind: TokenKind) -> bool {
        ffi::is_possible_parameter(kind.as_raw())
    }

    pub fn is_gate_type(kind: TokenKind) -> bool {
        ffi::is_gate_type(kind.as_raw())
    }

    pub fn is_port_direction(kind: TokenKind) -> bool {
        ffi::is_port_direction(kind.as_raw())
    }

    pub fn is_net_type(kind: TokenKind) -> bool {
        ffi::is_net_type(kind.as_raw())
    }

    pub fn get_integer_type(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_integer_type(kind.as_raw()))
    }

    pub fn get_keyword_type(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_keyword_type(kind.as_raw()))
    }

    pub fn get_procedural_block_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_procedural_block_kind(kind.as_raw()))
    }

    pub fn get_module_declaration_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_module_declaration_kind(kind.as_raw()))
    }

    pub fn is_possible_member_kind(token: TokenKind, member: SyntaxKind) -> bool {
        ffi::is_possible_member_kind(token.as_raw(), member.as_raw())
    }

    pub fn get_block_item_declaration_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_block_item_declaration_kind(kind.as_raw()))
    }

    pub fn get_library_map_member_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_library_map_member_kind(kind.as_raw()))
    }

    pub fn get_specify_item_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_specify_item_kind(kind.as_raw()))
    }

    pub fn get_config_header_item_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_config_header_item_kind(kind.as_raw()))
    }

    pub fn get_config_rule_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_raw(ffi::get_config_rule_kind(kind.as_raw()))
    }

    pub fn keyword_candidates_for_context(
        version: &str,
        context: SyntaxKeywordContext,
    ) -> Vec<String> {
        ffi::keyword_candidates_for_context(version, context as u8)
    }

    pub fn is_allowed_in_compilation_unit(kind: SyntaxKind) -> bool {
        ffi::is_allowed_in_compilation_unit(kind.as_u16())
    }

    pub fn is_allowed_in_generate(kind: SyntaxKind) -> bool {
        ffi::is_allowed_in_generate(kind.as_u16())
    }

    pub fn is_allowed_in_module(kind: SyntaxKind) -> bool {
        ffi::is_allowed_in_module(kind.as_u16())
    }

    pub fn is_allowed_in_interface(kind: SyntaxKind) -> bool {
        ffi::is_allowed_in_interface(kind.as_u16())
    }

    pub fn is_allowed_in_program(kind: SyntaxKind) -> bool {
        ffi::is_allowed_in_program(kind.as_u16())
    }

    pub fn is_allowed_in_package(kind: SyntaxKind) -> bool {
        ffi::is_allowed_in_package(kind.as_u16())
    }
}

impl SemanticFacts {
    pub fn is_edge_kind(kind: TokenKind) -> bool {
        ffi::is_edge_kind(kind.as_raw())
    }
}
