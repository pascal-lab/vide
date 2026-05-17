use smallvec::{SmallVec, smallvec};
use syntax::{ParserExpectedSyntax, SyntaxKeywordContext, SyntaxNode, SyntaxTree};
use utils::line_index::TextSize;

use super::{CompletionExpectation, ExpectationSource, ExpectedSyntax};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParserExpectations {
    items: SmallVec<[CompletionExpectation; 4]>,
    has_non_ansi_port: bool,
}

impl ParserExpectations {
    pub(super) fn items(&self) -> &[CompletionExpectation] {
        &self.items
    }

    pub(super) fn has_non_ansi_port(&self) -> bool {
        self.has_non_ansi_port
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

    if let Some(items) = items {
        for item in items {
            has_non_ansi_port |= item.name == "ExpectedNonAnsiPort";
            for expectation in map_item(item) {
                push_unique(&mut expectations, expectation);
            }
        }
    }

    normalize_config_phase(&mut expectations);

    ParserExpectations { items: expectations, has_non_ansi_port }
}

fn map_item(item: &ParserExpectedSyntax) -> SmallVec<[CompletionExpectation; 3]> {
    let source = ExpectationSource::Parser;
    match item.name.as_str() {
        "ExpectedParameterPort" => smallvec![CompletionExpectation {
            syntax: ExpectedSyntax::ParameterPortListItem,
            source,
        }],
        "ExpectedNonAnsiPort" => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::NonAnsiPortName, source }]
        }
        "ExpectedAnsiPort" => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::AnsiPortItem, source }]
        }
        "ExpectedFunctionPort" => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::FunctionPortItem, source }]
        }
        "ExpectedPortConnection" => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::PortConnection, source }]
        }
        "ExpectedArgument" => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::ArgumentExpr, source }]
        }
        "ExpectedExpression" => {
            smallvec![CompletionExpectation { syntax: ExpectedSyntax::Expression, source }]
        }
        "ExpectedStatement" => {
            let mut expectations = SmallVec::new();
            if let Some(context) = item.keyword_context {
                expectations.push(CompletionExpectation {
                    syntax: ExpectedSyntax::Keyword(context),
                    source,
                });
            }
            expectations.push(CompletionExpectation { syntax: ExpectedSyntax::Expression, source });
            expectations
        }
        _ => item
            .keyword_context
            .map(|context| {
                smallvec![CompletionExpectation {
                    syntax: ExpectedSyntax::Keyword(context),
                    source,
                }]
            })
            .unwrap_or_default(),
    }
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
