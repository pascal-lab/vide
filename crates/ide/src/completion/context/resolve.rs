use smallvec::SmallVec;

use super::{
    CompletionExpectation, ExpectationSource, ExpectedSyntax, TriggerChar,
    parser::ParserExpectations,
};
use crate::completion::{request::PortListKind, syntax_keywords};

pub(super) fn expectations(
    parser: ParserExpectations,
    local: Option<CompletionExpectation>,
    in_decl_name: bool,
    prefix: &str,
    trigger: Option<TriggerChar>,
) -> SmallVec<[CompletionExpectation; 4]> {
    let mut expectations = SmallVec::new();

    if let Some(expectation) = local.filter(|expectation| local_overrides(*expectation)) {
        push_unique(&mut expectations, expectation);
    } else if let Some(expectation) = port_keyword(&parser, prefix, trigger) {
        push_unique(&mut expectations, expectation);
    } else if in_decl_name {
        push_unique(
            &mut expectations,
            CompletionExpectation {
                syntax: ExpectedSyntax::DeclName,
                source: ExpectationSource::DeclarationName,
            },
        );
    } else {
        for expectation in parser.into_items() {
            push_unique(&mut expectations, expectation);
        }

        if let Some(expectation) = local {
            push_unique(&mut expectations, expectation);
        }
    }

    expectations
}

fn local_overrides(expectation: CompletionExpectation) -> bool {
    match expectation.syntax {
        ExpectedSyntax::ElseClause => false,
        ExpectedSyntax::DirectiveName
        | ExpectedSyntax::Keyword(_)
        | ExpectedSyntax::Expression
        | ExpectedSyntax::ParameterPortListItem
        | ExpectedSyntax::AnsiPortItem
        | ExpectedSyntax::FunctionPortItem
        | ExpectedSyntax::PortConnection
        | ExpectedSyntax::ArgumentExpr
        | ExpectedSyntax::NonAnsiPortName
        | ExpectedSyntax::DeclName => false,
        ExpectedSyntax::PortConnectionName
        | ExpectedSyntax::ParameterAssignmentName
        | ExpectedSyntax::MemberName
        | ExpectedSyntax::PortConnectionExpr
        | ExpectedSyntax::ParameterAssignmentExpr
        | ExpectedSyntax::AfterParamValueAssignmentHash
        | ExpectedSyntax::AfterParameterPortListHash
        | ExpectedSyntax::ParamValueAssignment
        | ExpectedSyntax::EventControl { .. } => true,
    }
}

fn port_keyword(
    parser: &ParserExpectations,
    prefix: &str,
    trigger: Option<TriggerChar>,
) -> Option<CompletionExpectation> {
    if prefix.is_empty() {
        if trigger == Some(TriggerChar::Comma) {
            return None;
        }

        return parser
            .items()
            .iter()
            .copied()
            .find(|expectation| port_kind(expectation.syntax).is_some());
    }

    parser
        .items()
        .iter()
        .copied()
        .find(|expectation| {
            port_kind(expectation.syntax)
                .is_some_and(|kind| syntax_keywords::has_port_item_keyword_prefix(prefix, kind))
        })
        .or_else(|| {
            (parser.has_non_ansi_port()
                && syntax_keywords::has_port_item_keyword_prefix(prefix, PortListKind::Ansi))
            .then_some(CompletionExpectation {
                syntax: ExpectedSyntax::AnsiPortItem,
                source: ExpectationSource::Parser,
            })
        })
}

fn port_kind(syntax: ExpectedSyntax) -> Option<PortListKind> {
    match syntax {
        ExpectedSyntax::AnsiPortItem => Some(PortListKind::Ansi),
        ExpectedSyntax::FunctionPortItem => Some(PortListKind::Function),
        _ => None,
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
