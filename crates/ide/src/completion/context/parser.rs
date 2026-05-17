use smallvec::{SmallVec, smallvec};
use syntax::{ParserExpectedSyntax, SyntaxNode, SyntaxTree};
use utils::line_index::TextSize;

use super::{CompletionExpectation, ExpectationSource, ExpectedSyntax};

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

pub(super) fn completion_expectations_for_parser_item(
    item: &ParserExpectedSyntax,
) -> SmallVec<[CompletionExpectation; 3]> {
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
