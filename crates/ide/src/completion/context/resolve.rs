use smallvec::SmallVec;
use syntax::diagnostics::SyntaxKeywordContext;

use super::{CompletionExpectation, ExpectedSyntax, TriggerChar, parser::ParserExpectations};
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
    } else if let Some(expectation) = dot_trigger_connection_name(&parser, trigger) {
        push_unique(&mut expectations, expectation);
    } else if let Some(expectation) = keyword_prefix(&parser, prefix) {
        push_unique(&mut expectations, expectation);
    } else if let Some(expectation) = port_keyword(&parser, prefix, trigger) {
        push_unique(&mut expectations, expectation);
    } else if in_decl_name {
        push_unique(&mut expectations, CompletionExpectation { syntax: ExpectedSyntax::DeclName });
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

fn dot_trigger_connection_name(
    parser: &ParserExpectations,
    trigger: Option<TriggerChar>,
) -> Option<CompletionExpectation> {
    if trigger != Some(TriggerChar::Dot) {
        return None;
    }

    parser.items().iter().find_map(|expectation| match expectation.syntax {
        ExpectedSyntax::PortConnection => {
            Some(CompletionExpectation { syntax: ExpectedSyntax::PortConnectionName })
        }
        _ => None,
    })
}

fn local_overrides(expectation: CompletionExpectation) -> bool {
    match expectation.syntax {
        ExpectedSyntax::ElseClause => false,
        ExpectedSyntax::DirectiveName
        | ExpectedSyntax::Keyword(_)
        | ExpectedSyntax::Expression
        | ExpectedSyntax::PortConnection
        | ExpectedSyntax::ArgumentExpr
        | ExpectedSyntax::NonAnsiPortName
        | ExpectedSyntax::DeclName
        | ExpectedSyntax::IntegerLiteralBase => false,
        ExpectedSyntax::ParameterPortListItem
        | ExpectedSyntax::AnsiPortItem
        | ExpectedSyntax::FunctionPortItem
        | ExpectedSyntax::PortConnectionName
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

fn keyword_prefix(parser: &ParserExpectations, prefix: &str) -> Option<CompletionExpectation> {
    if prefix.is_empty() {
        return None;
    }

    parser.items().iter().copied().find(|expectation| {
        keyword_context(expectation.syntax).is_some_and(|context| {
            !syntax_keywords::keyword_candidates_for_context(context, prefix).labels().is_empty()
        })
    })
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
            .then_some(CompletionExpectation { syntax: ExpectedSyntax::AnsiPortItem })
        })
}

fn keyword_context(syntax: ExpectedSyntax) -> Option<SyntaxKeywordContext> {
    match syntax {
        ExpectedSyntax::Keyword(context) => Some(context),
        ExpectedSyntax::ParameterPortListItem => Some(SyntaxKeywordContext::ParameterPortListItem),
        ExpectedSyntax::AnsiPortItem => Some(SyntaxKeywordContext::AnsiPortItem),
        ExpectedSyntax::FunctionPortItem => Some(SyntaxKeywordContext::FunctionPortItem),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use syntax::{
        TokenKind,
        diagnostics::{DiagCode, ParserExpectedSyntax},
    };

    use super::*;
    use crate::completion::context::parser;

    fn parser_item(code: DiagCode) -> ParserExpectedSyntax {
        ParserExpectedSyntax {
            code: code.code_raw(),
            subsystem: code.subsystem_raw(),
            name: code.info().expect("test diagnostic should have metadata").name.to_owned(),
            token_kind: TokenKind::UNKNOWN,
            keyword_context: None,
            location: None,
            end: None,
        }
    }

    fn first_syntax(
        items: Vec<ParserExpectedSyntax>,
        trigger: Option<TriggerChar>,
    ) -> Option<ExpectedSyntax> {
        let parser = parser::expectations(Some(&items));
        expectations(parser, None, false, "", trigger).first().map(|expectation| expectation.syntax)
    }

    #[test]
    fn dot_trigger_selects_port_connection_name_from_parser_port_connection() {
        assert_eq!(
            first_syntax(
                vec![parser_item(DiagCode::EXPECTED_PORT_CONNECTION)],
                Some(TriggerChar::Dot),
            ),
            Some(ExpectedSyntax::PortConnectionName)
        );
    }

    #[test]
    fn dot_trigger_leaves_argument_expr_alone() {
        assert_eq!(
            first_syntax(vec![parser_item(DiagCode::EXPECTED_ARGUMENT)], Some(TriggerChar::Dot)),
            Some(ExpectedSyntax::ArgumentExpr)
        );
    }
}
