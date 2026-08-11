use smallvec::{SmallVec, smallvec};
use syntax::{
    DiagCode, ParserExpectedSyntax, SyntaxKeywordContext, SyntaxNode, SyntaxTree, TokenKind,
};
use utils::line_index::TextSize;

use super::{CompletionExpectation, ExpectedSyntax};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParserExpectations {
    items: SmallVec<[CompletionExpectation; 4]>,
    has_non_ansi_port: bool,
    has_decl_name: bool,
}

impl ParserExpectations {
    pub(super) fn items(&self) -> &[CompletionExpectation] {
        &self.items
    }

    pub(super) fn has_non_ansi_port(&self) -> bool {
        self.has_non_ansi_port
    }

    pub(super) fn has_decl_name(&self) -> bool {
        self.has_decl_name
    }

    pub(super) fn into_items(self) -> SmallVec<[CompletionExpectation; 4]> {
        self.items
    }
}

pub(super) fn parser_expected_syntax_for_text(
    root: SyntaxNode<'_>,
    source_text: &str,
    offset: TextSize,
) -> Vec<ParserExpectedSyntax> {
    let offset = usize::from(offset);
    if root.kind() == syntax::SyntaxKind::LIBRARY_MAP {
        SyntaxTree::library_map_expected_syntax_at_offset(source_text, "source", "", offset)
    } else {
        SyntaxTree::expected_syntax_at_offset(source_text, "source", "", offset)
    }
}

pub(super) fn expectations(items: Option<&[ParserExpectedSyntax]>) -> ParserExpectations {
    let mut expectations = SmallVec::new();
    let mut has_non_ansi_port = false;
    let mut has_decl_name = false;

    if let Some(items) = items {
        for item in items {
            has_non_ansi_port |= item.diagnostic_code() == DiagCode::EXPECTED_NON_ANSI_PORT;
            has_decl_name |= is_decl_name_expectation(item);
            for expectation in map_item(item) {
                push_unique(&mut expectations, expectation);
            }
        }
    }

    normalize_config_phase(&mut expectations);

    ParserExpectations { items: expectations, has_non_ansi_port, has_decl_name }
}

fn map_item(item: &ParserExpectedSyntax) -> SmallVec<[CompletionExpectation; 3]> {
    match item.diagnostic_code() {
        code if code == DiagCode::EXPECTED_PARAMETER_PORT => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::ParameterPortListItem }]
        }
        code if code == DiagCode::EXPECTED_NON_ANSI_PORT => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::NonAnsiPortName }]
        }
        code if code == DiagCode::EXPECTED_ANSI_PORT => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::AnsiPortItem }]
        }
        code if code == DiagCode::EXPECTED_FUNCTION_PORT => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::FunctionPortItem }]
        }
        code if code == DiagCode::EXPECTED_PORT_CONNECTION => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::PortConnection }]
        }
        code if code == DiagCode::EXPECTED_ARGUMENT => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::ArgumentExpr }]
        }
        code if code == DiagCode::EXPECTED_EXPRESSION => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::Expression }]
        }
        code if code == DiagCode::EXPECTED_STATEMENT => {
            let mut expectations = SmallVec::new();
            if let Some(context) = item.keyword_context {
                expectations
                    .push(CompletionExpectation { syntax: ExpectedSyntax::Keyword(context) });
            }
            expectations.push(CompletionExpectation { syntax: ExpectedSyntax::Expression });
            expectations
        }
        _ => item
            .keyword_context
            .map(|context| {
                smallvec![CompletionExpectation { syntax: ExpectedSyntax::Keyword(context) }]
            })
            .unwrap_or_default(),
    }
}

fn is_decl_name_expectation(item: &ParserExpectedSyntax) -> bool {
    matches!(
        item.diagnostic_code(),
        DiagCode::EXPECTED_IDENTIFIER
            | DiagCode::EXPECTED_DECLARATOR
            | DiagCode::EXPECTED_SUBROUTINE_NAME
    ) || (item.diagnostic_code() == DiagCode::EXPECTED_TOKEN
        && item.token_kind == TokenKind::IDENTIFIER)
}

fn normalize_config_phase(expectations: &mut SmallVec<[CompletionExpectation; 4]>) {
    let has_header = expectations.iter().any(|expectation| {
        expectation.syntax == ExpectedSyntax::Keyword(SyntaxKeywordContext::ConfigHeaderItem)
    });
    if has_header {
        expectations.retain(|expectation| {
            expectation.syntax != ExpectedSyntax::Keyword(SyntaxKeywordContext::ConfigRule)
        });
    }
}

fn push_unique(
    expectations: &mut SmallVec<[CompletionExpectation; 4]>,
    expectation: CompletionExpectation,
) {
    if !expectations.iter().any(|existing| existing.syntax == expectation.syntax) {
        expectations.push(expectation);
    }
}
