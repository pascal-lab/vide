use crate::{CxxSV, SyntaxKind, TokenKind, ffi};

pub struct SemanticFacts;
pub struct SyntaxFacts;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxKeywordContext {
    CompilationUnitMember,
    LibraryMapMember,
    ModuleHeaderItem,
    ModuleMember,
    GenerateMember,
    SpecifyItem,
    ConfigHeaderItem,
    ConfigRule,
    BlockItem,
    Statement,
    ParameterPortListItem,
    AnsiPortItem,
    FunctionPortItem,
    GateType,
}

impl SyntaxKeywordContext {
    const VALUES: [Self; 14] = [
        Self::CompilationUnitMember,
        Self::LibraryMapMember,
        Self::ModuleHeaderItem,
        Self::ModuleMember,
        Self::GenerateMember,
        Self::SpecifyItem,
        Self::ConfigHeaderItem,
        Self::ConfigRule,
        Self::BlockItem,
        Self::Statement,
        Self::ParameterPortListItem,
        Self::AnsiPortItem,
        Self::FunctionPortItem,
        Self::GateType,
    ];

    #[inline]
    pub(crate) fn from_raw(value: u8) -> Option<Self> {
        Self::VALUES.get(value as usize).copied()
    }
}

impl SyntaxFacts {
    #[inline]
    pub fn is_possible_statement(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_statement(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_expression(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_expression(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_data_type(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_data_type(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_argument(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_argument(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_param_assignment(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_param_assignment(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_port_connection(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_port_connection(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_ansi_port(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_ansi_port(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_non_ansi_port(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_non_ansi_port(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_function_port(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_function_port(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_parameter(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_parameter(kind.as_u16())
    }

    #[inline]
    pub fn is_gate_type(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_gate_type(kind.as_u16())
    }

    #[inline]
    pub fn is_port_direction(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_port_direction(kind.as_u16())
    }

    #[inline]
    pub fn is_net_type(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_net_type(kind.as_u16())
    }

    #[inline]
    pub fn get_integer_type(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_integer_type(kind.as_u16()))
    }

    #[inline]
    pub fn get_keyword_type(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_keyword_type(kind.as_u16()))
    }

    #[inline]
    pub fn get_procedural_block_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_procedural_block_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_module_declaration_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_module_declaration_kind(kind.as_u16()))
    }

    #[inline]
    pub fn is_possible_member_kind(token_kind: TokenKind, member_kind: SyntaxKind) -> bool {
        ffi::SyntaxToken::is_possible_member_kind(token_kind.as_u16(), member_kind.as_u16())
    }

    #[inline]
    pub fn get_block_item_declaration_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_block_item_declaration_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_library_map_member_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_library_map_member_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_specify_item_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_specify_item_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_config_header_item_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_config_header_item_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_config_rule_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_config_rule_kind(kind.as_u16()))
    }

    #[inline]
    pub fn keyword_candidates_for_context(
        version: &str,
        context: SyntaxKeywordContext,
    ) -> Vec<String> {
        ffi::SyntaxToken::keyword_candidates_for_context(CxxSV::new(version), context as u8)
    }

    #[inline]
    pub fn is_allowed_in_compilation_unit(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_compilation_unit(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_generate(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_generate(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_module(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_module(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_interface(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_interface(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_program(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_program(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_package(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_package(kind.as_u16())
    }
}

impl SemanticFacts {
    #[inline]
    pub fn is_edge_kind(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_edge_kind(kind.as_u16())
    }
}
