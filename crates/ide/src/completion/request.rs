use super::context::{CompletionContext, ExpectedSyntax, LexContext, TriggerChar};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HashKind {
    ParamValueAssignment,
    ParameterPortList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParenListKind {
    ParamValueAssignment,
    ParameterPortList,
    PortConnections,
    Arguments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortListKind {
    Ansi,
    Function,
    NonAnsi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionRequest {
    Directives,
    Keywords(ExpectedSyntax),
    Expression,
    PortConnectionName,
    ParameterAssignmentName,
    MemberName,
    PortConnectionExpr,
    ParameterAssignmentExpr,
    AfterHash(HashKind),
    ParenList(ParenListKind),
    PortList(PortListKind),
    EventControl { wrap_in_parens: bool },
}

impl CompletionRequest {
    pub(crate) fn from_context(ctx: &CompletionContext) -> Option<Self> {
        if matches!(
            ctx.expectation.map(|expectation| expectation.syntax),
            Some(ExpectedSyntax::DirectiveName)
        ) {
            return Some(Self::Directives);
        }

        if ctx.lex != LexContext::Code {
            return None;
        }

        let expected = ctx.expectation?.syntax;
        let request = request_for_expected_syntax(expected)?;

        if newline_trigger_outside_request(ctx, request) {
            return None;
        }

        if punctuation_trigger_without_specific_request(ctx, request) {
            return None;
        }

        Some(request)
    }
}

fn request_for_expected_syntax(expected: ExpectedSyntax) -> Option<CompletionRequest> {
    Some(match expected {
        ExpectedSyntax::DirectiveName => CompletionRequest::Directives,
        ExpectedSyntax::DeclName => return None,
        ExpectedSyntax::CompilationUnitItem
        | ExpectedSyntax::ModuleHeaderItem
        | ExpectedSyntax::ModuleItem
        | ExpectedSyntax::GenerateItem
        | ExpectedSyntax::SpecifyItem
        | ExpectedSyntax::ConfigItem { .. }
        | ExpectedSyntax::BlockItem { .. }
        | ExpectedSyntax::Statement => CompletionRequest::Keywords(expected),
        ExpectedSyntax::Expression => CompletionRequest::Expression,
        ExpectedSyntax::PortConnectionName => CompletionRequest::PortConnectionName,
        ExpectedSyntax::ParameterAssignmentName => CompletionRequest::ParameterAssignmentName,
        ExpectedSyntax::MemberName => CompletionRequest::MemberName,
        ExpectedSyntax::PortConnectionExpr => CompletionRequest::PortConnectionExpr,
        ExpectedSyntax::ParameterAssignmentExpr => CompletionRequest::ParameterAssignmentExpr,
        ExpectedSyntax::AfterParamValueAssignmentHash => {
            CompletionRequest::AfterHash(HashKind::ParamValueAssignment)
        }
        ExpectedSyntax::AfterParameterPortListHash => {
            CompletionRequest::AfterHash(HashKind::ParameterPortList)
        }
        ExpectedSyntax::ParamValueAssignment => {
            CompletionRequest::ParenList(ParenListKind::ParamValueAssignment)
        }
        ExpectedSyntax::ParameterPortListItem => {
            CompletionRequest::ParenList(ParenListKind::ParameterPortList)
        }
        ExpectedSyntax::PortConnection => {
            CompletionRequest::ParenList(ParenListKind::PortConnections)
        }
        ExpectedSyntax::ArgumentExpr => CompletionRequest::ParenList(ParenListKind::Arguments),
        ExpectedSyntax::AnsiPortItem => CompletionRequest::PortList(PortListKind::Ansi),
        ExpectedSyntax::FunctionPortItem => CompletionRequest::PortList(PortListKind::Function),
        ExpectedSyntax::NonAnsiPortName => CompletionRequest::PortList(PortListKind::NonAnsi),
        ExpectedSyntax::EventControl { wrap_in_parens } => {
            CompletionRequest::EventControl { wrap_in_parens }
        }
    })
}

fn newline_trigger_outside_request(ctx: &CompletionContext, request: CompletionRequest) -> bool {
    ctx.trigger == Some(TriggerChar::Newline) && !request.accepts_newline_trigger()
}

fn punctuation_trigger_without_specific_request(
    ctx: &CompletionContext,
    request: CompletionRequest,
) -> bool {
    ctx.trigger.is_some()
        && request.is_punctuation_trigger_suppressed()
        && ctx.prefix.is_empty()
        && ctx.replacement.is_empty()
}

impl CompletionRequest {
    fn accepts_newline_trigger(self) -> bool {
        matches!(self, CompletionRequest::PortList(PortListKind::Ansi | PortListKind::Function))
    }

    fn is_punctuation_trigger_suppressed(self) -> bool {
        matches!(self, CompletionRequest::Keywords(_))
    }
}

#[cfg(test)]
mod tests {
    use utils::line_index::{TextRange, TextSize};

    use super::*;
    use crate::completion::context::{CompletionExpectation, ExpectationSource};

    fn context(
        lex: LexContext,
        trigger: Option<TriggerChar>,
        syntax: Option<ExpectedSyntax>,
    ) -> CompletionContext {
        CompletionContext {
            replacement: TextRange::empty(TextSize::from(0)),
            prefix: String::new(),
            trigger,
            lex,
            expectation: syntax.map(|syntax| CompletionExpectation {
                syntax,
                source: ExpectationSource::RecoveredSyntax,
            }),
            in_decl_name: false,
        }
    }

    #[test]
    fn maps_syntax_expectations_to_provider_requests() {
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                None,
                Some(ExpectedSyntax::ModuleItem)
            )),
            Some(CompletionRequest::Keywords(ExpectedSyntax::ModuleItem))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                None,
                Some(ExpectedSyntax::PortConnection)
            )),
            Some(CompletionRequest::ParenList(ParenListKind::PortConnections))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                None,
                Some(ExpectedSyntax::EventControl { wrap_in_parens: true })
            )),
            Some(CompletionRequest::EventControl { wrap_in_parens: true })
        );
    }

    #[test]
    fn keeps_directive_requests_outside_regular_code() {
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::PreprocDirective,
                None,
                Some(ExpectedSyntax::DirectiveName)
            )),
            Some(CompletionRequest::Directives)
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::LineComment,
                None,
                Some(ExpectedSyntax::ModuleItem)
            )),
            None
        );
    }

    #[test]
    fn suppresses_broad_keyword_requests_for_empty_punctuation_triggers() {
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                Some(TriggerChar::Comma),
                Some(ExpectedSyntax::ModuleItem)
            )),
            None
        );
    }

    #[test]
    fn accepts_newline_only_for_port_item_requests() {
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                Some(TriggerChar::Newline),
                Some(ExpectedSyntax::AnsiPortItem)
            )),
            Some(CompletionRequest::PortList(PortListKind::Ansi))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                Some(TriggerChar::Newline),
                Some(ExpectedSyntax::ModuleItem)
            )),
            None
        );
    }
}
