use smallvec::{SmallVec, smallvec};
use syntax::SyntaxKeywordContext;

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
pub(crate) enum KeywordSnippetScope {
    None,
    CompilationUnit,
    LibraryMap,
    DesignItem,
    ParameterPortList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeywordProvider {
    pub(crate) context: SyntaxKeywordContext,
    pub(crate) snippets: KeywordSnippetScope,
    pub(crate) module_instantiations: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionRequest {
    providers: SmallVec<[ProviderPlan; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderPlan {
    pub(crate) provider: CompletionProvider,
    trigger: TriggerPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerPolicy {
    NonNewline,
    ManualOrPrefix,
    ManualPrefixOrNewline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionProvider {
    Directives,
    Keywords(KeywordProvider),
    SystemTasks,
    Expression,
    PortConnectionName,
    ParameterAssignmentName,
    MemberName,
    PortConnectionExpr,
    ParameterAssignmentExpr,
    ElseClause,
    AfterHash(HashKind),
    ParenList(ParenListKind),
    PortList(PortListKind),
    EventControl { wrap_in_parens: bool },
}

impl CompletionRequest {
    fn single(provider: CompletionProvider) -> Self {
        Self::from_providers(smallvec![provider])
    }

    fn from_providers(providers: SmallVec<[CompletionProvider; 2]>) -> Self {
        Self { providers: providers.into_iter().map(ProviderPlan::new).collect() }
    }

    pub(crate) fn providers(&self) -> impl Iterator<Item = CompletionProvider> + '_ {
        self.providers.iter().map(|plan| plan.provider)
    }

    pub(crate) fn from_context(ctx: &CompletionContext) -> Option<Self> {
        if ctx
            .expectations
            .iter()
            .any(|expectation| expectation.syntax == ExpectedSyntax::DirectiveName)
        {
            return Some(Self::single(CompletionProvider::Directives));
        }

        if ctx.lex != LexContext::Code {
            return None;
        }

        let mut request = CompletionRequest { providers: SmallVec::new() };
        for expectation in &ctx.expectations {
            if let Some(expected_request) = request_for_expected_syntax(expectation.syntax) {
                for plan in expected_request.providers {
                    request.push_plan(plan);
                }
            }
        }
        request.activated_by(ctx)
    }

    fn activated_by(mut self, ctx: &CompletionContext) -> Option<Self> {
        self.providers.retain(|plan| plan.accepts_context(ctx));
        (!self.providers.is_empty()).then_some(self)
    }

    fn push_plan(&mut self, plan: ProviderPlan) {
        if !self.providers.iter().any(|existing| existing.provider == plan.provider) {
            self.providers.push(plan);
        }
    }
}

impl ProviderPlan {
    fn new(provider: CompletionProvider) -> Self {
        Self { provider, trigger: provider.trigger_policy() }
    }

    fn accepts_context(self, ctx: &CompletionContext) -> bool {
        self.trigger.allows(ctx)
    }
}

fn request_for_expected_syntax(expected: ExpectedSyntax) -> Option<CompletionRequest> {
    let provider = match expected {
        ExpectedSyntax::DirectiveName => CompletionProvider::Directives,
        ExpectedSyntax::DeclName => return None,
        ExpectedSyntax::Keyword(context) => {
            let keyword_provider =
                CompletionProvider::Keywords(keyword_provider_for_context(context));
            if matches!(context, SyntaxKeywordContext::BlockItem | SyntaxKeywordContext::Statement)
            {
                return Some(CompletionRequest::from_providers(smallvec![
                    keyword_provider,
                    CompletionProvider::SystemTasks,
                    CompletionProvider::Expression,
                ]));
            }
            keyword_provider
        }
        ExpectedSyntax::Expression => CompletionProvider::Expression,
        ExpectedSyntax::PortConnectionName => CompletionProvider::PortConnectionName,
        ExpectedSyntax::ParameterAssignmentName => CompletionProvider::ParameterAssignmentName,
        ExpectedSyntax::MemberName => CompletionProvider::MemberName,
        ExpectedSyntax::PortConnectionExpr => CompletionProvider::PortConnectionExpr,
        ExpectedSyntax::ParameterAssignmentExpr => CompletionProvider::ParameterAssignmentExpr,
        ExpectedSyntax::ElseClause => CompletionProvider::ElseClause,
        ExpectedSyntax::AfterParamValueAssignmentHash => {
            CompletionProvider::AfterHash(HashKind::ParamValueAssignment)
        }
        ExpectedSyntax::AfterParameterPortListHash => {
            CompletionProvider::AfterHash(HashKind::ParameterPortList)
        }
        ExpectedSyntax::ParamValueAssignment => {
            CompletionProvider::ParenList(ParenListKind::ParamValueAssignment)
        }
        ExpectedSyntax::ParameterPortListItem => {
            return Some(CompletionRequest::from_providers(smallvec![
                CompletionProvider::ParenList(ParenListKind::ParameterPortList),
                CompletionProvider::Keywords(keyword_provider_for_context(
                    SyntaxKeywordContext::ParameterPortListItem,
                )),
            ]));
        }
        ExpectedSyntax::PortConnection => {
            CompletionProvider::ParenList(ParenListKind::PortConnections)
        }
        ExpectedSyntax::ArgumentExpr => CompletionProvider::ParenList(ParenListKind::Arguments),
        ExpectedSyntax::AnsiPortItem => {
            return Some(CompletionRequest::from_providers(smallvec![
                CompletionProvider::PortList(PortListKind::Ansi),
                CompletionProvider::Keywords(keyword_provider_for_context(
                    SyntaxKeywordContext::AnsiPortItem,
                )),
            ]));
        }
        ExpectedSyntax::FunctionPortItem => {
            return Some(CompletionRequest::from_providers(smallvec![
                CompletionProvider::PortList(PortListKind::Function),
                CompletionProvider::Keywords(keyword_provider_for_context(
                    SyntaxKeywordContext::FunctionPortItem,
                )),
            ]));
        }
        ExpectedSyntax::NonAnsiPortName => CompletionProvider::PortList(PortListKind::NonAnsi),
        ExpectedSyntax::EventControl { wrap_in_parens } => {
            CompletionProvider::EventControl { wrap_in_parens }
        }
    };

    Some(CompletionRequest::single(provider))
}

fn keyword_provider_for_context(context: SyntaxKeywordContext) -> KeywordProvider {
    let snippets = match context {
        SyntaxKeywordContext::CompilationUnitMember => KeywordSnippetScope::CompilationUnit,
        SyntaxKeywordContext::LibraryMapMember => KeywordSnippetScope::LibraryMap,
        SyntaxKeywordContext::ModuleMember
        | SyntaxKeywordContext::GenerateMember
        | SyntaxKeywordContext::SpecifyItem
        | SyntaxKeywordContext::BlockItem
        | SyntaxKeywordContext::Statement => KeywordSnippetScope::DesignItem,
        SyntaxKeywordContext::ModuleHeaderItem
        | SyntaxKeywordContext::ConfigHeaderItem
        | SyntaxKeywordContext::ConfigRule
        | SyntaxKeywordContext::GateType => KeywordSnippetScope::None,
        SyntaxKeywordContext::ParameterPortListItem => KeywordSnippetScope::ParameterPortList,
        SyntaxKeywordContext::AnsiPortItem | SyntaxKeywordContext::FunctionPortItem => {
            KeywordSnippetScope::None
        }
    };

    let module_instantiations = matches!(
        context,
        SyntaxKeywordContext::ModuleMember | SyntaxKeywordContext::GenerateMember
    );

    KeywordProvider { context, snippets, module_instantiations }
}

impl CompletionProvider {
    fn trigger_policy(self) -> TriggerPolicy {
        match self {
            CompletionProvider::Keywords(provider) => provider.trigger_policy(),
            CompletionProvider::SystemTasks => TriggerPolicy::ManualOrPrefix,
            CompletionProvider::ElseClause => TriggerPolicy::ManualOrPrefix,
            CompletionProvider::PortList(PortListKind::Ansi | PortListKind::Function) => {
                TriggerPolicy::ManualPrefixOrNewline
            }
            _ => TriggerPolicy::NonNewline,
        }
    }
}

impl TriggerPolicy {
    fn allows(self, ctx: &CompletionContext) -> bool {
        match ctx.trigger {
            None => true,
            Some(TriggerChar::Newline) => matches!(self, TriggerPolicy::ManualPrefixOrNewline),
            Some(_) if ctx.prefix.is_empty() && ctx.replacement.is_empty() => {
                matches!(self, TriggerPolicy::NonNewline)
            }
            Some(_) => true,
        }
    }
}

impl KeywordProvider {
    fn trigger_policy(self) -> TriggerPolicy {
        if matches!(
            self.context,
            SyntaxKeywordContext::AnsiPortItem | SyntaxKeywordContext::FunctionPortItem
        ) {
            TriggerPolicy::ManualPrefixOrNewline
        } else {
            TriggerPolicy::ManualOrPrefix
        }
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
            expectations: syntax.map_or_else(SmallVec::new, |syntax| {
                smallvec![CompletionExpectation {
                    syntax,
                    source: ExpectationSource::RecoveredSyntax,
                }]
            }),
            in_decl_name: false,
        }
    }

    fn keyword(context: SyntaxKeywordContext) -> ExpectedSyntax {
        ExpectedSyntax::Keyword(context)
    }

    #[test]
    fn maps_syntax_expectations_to_provider_requests() {
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                None,
                Some(keyword(SyntaxKeywordContext::ModuleMember))
            )),
            Some(CompletionRequest::single(CompletionProvider::Keywords(KeywordProvider {
                context: SyntaxKeywordContext::ModuleMember,
                snippets: KeywordSnippetScope::DesignItem,
                module_instantiations: true,
            })))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                None,
                Some(ExpectedSyntax::PortConnection)
            )),
            Some(CompletionRequest::single(CompletionProvider::ParenList(
                ParenListKind::PortConnections
            )))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                None,
                Some(ExpectedSyntax::ParameterPortListItem)
            )),
            Some(CompletionRequest::from_providers(smallvec![
                CompletionProvider::ParenList(ParenListKind::ParameterPortList),
                CompletionProvider::Keywords(KeywordProvider {
                    context: SyntaxKeywordContext::ParameterPortListItem,
                    snippets: KeywordSnippetScope::ParameterPortList,
                    module_instantiations: false,
                }),
            ]))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                None,
                Some(keyword(SyntaxKeywordContext::Statement))
            )),
            Some(CompletionRequest::from_providers(smallvec![
                CompletionProvider::Keywords(KeywordProvider {
                    context: SyntaxKeywordContext::Statement,
                    snippets: KeywordSnippetScope::DesignItem,
                    module_instantiations: false,
                }),
                CompletionProvider::SystemTasks,
                CompletionProvider::Expression,
            ]))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                None,
                Some(ExpectedSyntax::EventControl { wrap_in_parens: true })
            )),
            Some(CompletionRequest::single(CompletionProvider::EventControl {
                wrap_in_parens: true
            }))
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
            Some(CompletionRequest::single(CompletionProvider::Directives))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::LineComment,
                None,
                Some(keyword(SyntaxKeywordContext::ModuleMember))
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
                Some(keyword(SyntaxKeywordContext::ModuleMember))
            )),
            None
        );
    }

    #[test]
    fn trigger_activation_filters_each_provider() {
        let request = CompletionRequest {
            providers: smallvec![
                ProviderPlan::new(CompletionProvider::Keywords(KeywordProvider {
                    context: SyntaxKeywordContext::ModuleMember,
                    snippets: KeywordSnippetScope::DesignItem,
                    module_instantiations: true,
                })),
                ProviderPlan::new(CompletionProvider::PortConnectionName),
            ],
        };
        let activated = request.activated_by(&context(
            LexContext::Code,
            Some(TriggerChar::Comma),
            Some(keyword(SyntaxKeywordContext::ModuleMember)),
        ));

        assert_eq!(
            activated,
            Some(CompletionRequest::single(CompletionProvider::PortConnectionName))
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
            Some(CompletionRequest::from_providers(smallvec![
                CompletionProvider::PortList(PortListKind::Ansi),
                CompletionProvider::Keywords(KeywordProvider {
                    context: SyntaxKeywordContext::AnsiPortItem,
                    snippets: KeywordSnippetScope::None,
                    module_instantiations: false,
                }),
            ]))
        );
        assert_eq!(
            CompletionRequest::from_context(&context(
                LexContext::Code,
                Some(TriggerChar::Newline),
                Some(keyword(SyntaxKeywordContext::ModuleMember))
            )),
            None
        );
    }
}
