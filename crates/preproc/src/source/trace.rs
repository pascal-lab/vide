use std::collections::BTreeMap;

use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SourceBufferOrigin, SourceBufferRange, SyntaxKind,
    preproc::{
        ActualArgument, EmittedToken, Event, EventId, MacroArgumentOrigin, MacroBodyOrigin,
        MacroBuiltinOrigin, MacroCallId, MacroDefinitionId, MacroExpansionId, MacroOperationOrigin,
        MacroParam, Token, TokenOrigin, Trace,
    },
};
use utils::line_index::{TextRange, TextSize};

use super::*;

impl From<EventId> for SourcePreprocEventId {
    fn from(value: EventId) -> Self {
        Self(value.0)
    }
}

impl From<MacroDefinitionId> for SourceMacroDefinitionKey {
    fn from(value: MacroDefinitionId) -> Self {
        Self::new(value.0)
    }
}

impl From<MacroCallId> for SourceMacroCallKey {
    fn from(value: MacroCallId) -> Self {
        Self::new(value.0)
    }
}

impl From<MacroExpansionId> for SourceMacroExpansionKey {
    fn from(value: MacroExpansionId) -> Self {
        Self::new(value.0)
    }
}

impl From<MacroBodyOrigin> for SourceMacroBodyIdentity {
    fn from(value: MacroBodyOrigin) -> Self {
        Self {
            call: SourceMacroCallKey::from(value.call_id),
            definition: SourceMacroDefinitionKey::from(value.definition_id),
            expansion: SourceMacroExpansionKey::from(value.expansion_id),
            parent_expansion: value.parent_expansion_id.map(SourceMacroExpansionKey::from),
            body_token_index: value.body_token_index as usize,
        }
    }
}

impl From<MacroArgumentOrigin> for SourceMacroArgumentIdentity {
    fn from(value: MacroArgumentOrigin) -> Self {
        Self {
            call: SourceMacroCallKey::from(value.call_id),
            definition: SourceMacroDefinitionKey::from(value.definition_id),
            expansion: SourceMacroExpansionKey::from(value.expansion_id),
            parent_expansion: value.parent_expansion_id.map(SourceMacroExpansionKey::from),
            body_token_index: value.body_token_index as usize,
            argument_index: value.argument_index as usize,
            argument_token_index: value.argument_token_index as usize,
        }
    }
}

impl From<MacroBuiltinOrigin> for SourceMacroBuiltinIdentity {
    fn from(value: MacroBuiltinOrigin) -> Self {
        Self {
            call: SourceMacroCallKey::from(value.call_id),
            expansion: SourceMacroExpansionKey::from(value.expansion_id),
            parent_expansion: value.parent_expansion_id.map(SourceMacroExpansionKey::from),
        }
    }
}

impl From<MacroOperationOrigin> for SourceMacroOperationIdentity {
    fn from(value: MacroOperationOrigin) -> Self {
        Self {
            call: SourceMacroCallKey::from(value.call_id),
            definition: SourceMacroDefinitionKey::from(value.definition_id),
            expansion: SourceMacroExpansionKey::from(value.expansion_id),
            parent_expansion: value.parent_expansion_id.map(SourceMacroExpansionKey::from),
            body_token_index: value.body_token_index as usize,
            argument_index: value.argument_index.map(|index| index as usize),
            argument_token_index: value.argument_token_index.map(|index| index as usize),
        }
    }
}

impl SourcePreprocIndex {
    pub fn from_trace(trace: Trace) -> Result<Self, SourcePreprocError> {
        let root_source = PreprocSourceId::from(trace.root_buffer_id);
        let include_edges = trace
            .include_edges
            .iter()
            .map(|edge| SourceIncludeEdge {
                include_event_id: SourcePreprocEventId::from(edge.include_event_id),
                included_source: PreprocSourceId::from(edge.included_buffer_id),
            })
            .collect::<Vec<_>>();
        let included_by = include_edges
            .iter()
            .map(|edge| (edge.included_source, edge.include_event_id))
            .collect::<BTreeMap<_, _>>();
        let emitted_tokens = trace.emitted_tokens;
        let mut index = Self {
            root_source: Some(root_source),
            sources: trace
                .source_buffers
                .into_iter()
                .map(|source| PreprocSource {
                    id: { PreprocSourceId::from(source.buffer_id) },
                    path: source.path.to_smolstr(),
                    origin: source_origin(
                        PreprocSourceId::from(source.buffer_id),
                        root_source,
                        source.origin,
                        &included_by,
                    ),
                })
                .collect(),
            include_edges,
            ..Self::default()
        };

        if !index.sources.iter().any(|source| source.id == root_source) {
            return Err(SourcePreprocError::MissingRootSource);
        }

        for (source_order, directive) in trace.events.into_iter().enumerate() {
            collect_trace_event(&mut index, source_order, directive)?;
        }
        index.emitted_tokens = emitted_tokens.into_iter().map(emitted_token_from_trace).collect();

        Ok(index)
    }
}

fn source_origin(
    source: PreprocSourceId,
    root_source: PreprocSourceId,
    origin: SourceBufferOrigin,
    included_by: &BTreeMap<PreprocSourceId, SourcePreprocEventId>,
) -> PreprocSourceOrigin {
    if source == root_source {
        return PreprocSourceOrigin::Root;
    }

    if origin == SourceBufferOrigin::Predefine {
        return PreprocSourceOrigin::Predefine;
    }

    included_by
        .get(&source)
        .copied()
        .map(|include_event_id| PreprocSourceOrigin::Included { include_event_id })
        .unwrap_or(PreprocSourceOrigin::Detached)
}

fn collect_trace_event(
    index: &mut SourcePreprocIndex,
    source_order: usize,
    directive: Event,
) -> Result<(), SourcePreprocError> {
    index.inactive_ranges.extend(
        directive
            .disabled_ranges
            .iter()
            .filter_map(source_range_from_trace)
            .filter(|range| !range.range.is_empty()),
    );

    let Some(kind) = event_kind(directive.kind) else {
        return Ok(());
    };
    let event_id = SourcePreprocEventId::from(directive.event_id);
    let range = required_event_range(source_order, kind, &directive)?;

    match kind {
        MacroEventKind::Define => {
            let event_index = index.defines.len();
            let define = collect_trace_define(directive, event_id, range);
            index.defines.push(define);
            push_source_event_record(index, event_id, kind, event_index, range);
        }
        MacroEventKind::Undef => {
            let event_index = index.undefs.len();
            index.undefs.push(SourceMacroUndef {
                event_id,
                name: directive.name.value(),
                name_range: directive.name.source_range(),
                range,
            });
            push_source_event_record(index, event_id, kind, event_index, range);
        }
        MacroEventKind::Include => {
            let event_index = index.includes.len();
            let target = directive.include_file_name.include_target();
            index.includes.push(SourceMacroInclude {
                event_id,
                target,
                target_range: directive.include_file_name.source_range(),
                range,
            });
            push_source_event_record(index, event_id, kind, event_index, range);
        }
        MacroEventKind::Conditional | MacroEventKind::Branch => {
            let event_index = index.conditionals.len();
            index.conditionals.push(SourceMacroConditional {
                event_id,
                kind: trace_conditional_kind(directive.kind),
                expr: directive.expr_tokens.into_iter().map(macro_token_from_trace).collect(),
                range,
            });
            push_source_event_record(index, event_id, kind, event_index, range);
        }
        MacroEventKind::Usage => {
            let event_index = index.usages.len();
            index.usages.push(SourceMacroUsage {
                event_id,
                identity: directive.macro_call_id.map(SourceMacroCallKey::from),
                definition_identity: directive
                    .macro_definition_id
                    .map(SourceMacroDefinitionKey::from),
                expansion_identity: directive.macro_expansion_id.map(SourceMacroExpansionKey::from),
                parent_expansion_identity: directive
                    .parent_macro_expansion_id
                    .map(SourceMacroExpansionKey::from),
                name: directive.name.macro_name(),
                name_range: directive.name.source_range(),
                arguments: directive
                    .arguments
                    .into_iter()
                    .enumerate()
                    .map(macro_actual_argument_from_trace)
                    .collect(),
                range,
            });
            push_source_event_record(index, event_id, kind, event_index, range);
        }
    }

    Ok(())
}

fn collect_trace_define(
    directive: Event,
    event_id: SourcePreprocEventId,
    range: SourceRange,
) -> SourceMacroDefine {
    SourceMacroDefine {
        event_id,
        identity: directive.macro_definition_id.map(SourceMacroDefinitionKey::from),
        name: directive.name.value(),
        name_range: directive.name.source_range(),
        params: (!directive.params.is_empty())
            .then(|| directive.params.into_iter().map(macro_param_from_trace).collect()),
        body: directive.body_tokens.into_iter().map(macro_token_from_trace).collect(),
        range,
    }
}

fn macro_param_from_trace(param: MacroParam) -> SourceMacroParam {
    SourceMacroParam {
        name: param.name.value(),
        name_range: param.name.source_range(),
        default: param
            .default_tokens
            .map(|tokens| tokens.into_iter().map(macro_token_from_trace).collect()),
        range: trace_range(&param.range),
    }
}

fn macro_actual_argument_from_trace(
    (argument_index, argument): (usize, ActualArgument),
) -> SourceMacroActualArgument {
    SourceMacroActualArgument {
        argument_index,
        argument_range: trace_range(&argument.range),
        tokens: argument.tokens.into_iter().map(macro_token_from_trace).collect(),
    }
}

fn macro_token_from_trace(token: Token) -> SourceMacroToken {
    SourceMacroToken {
        raw: token.raw_text.to_smolstr(),
        value: token.value_text.to_smolstr(),
        range: trace_range(&token.range),
    }
}

fn emitted_token_from_trace(token: EmittedToken) -> SourceEmittedTokenFact {
    SourceEmittedTokenFact {
        raw: token.raw_text.to_smolstr(),
        value: token.value_text.to_smolstr(),
        display: token.display_text.to_smolstr(),
        kind: SourceTokenKind::Syntax(token.token_kind),
        provenance: emitted_token_provenance_from_trace(token.provenance),
    }
}

fn emitted_token_provenance_from_trace(provenance: TokenOrigin) -> SourceTokenProvenanceFact {
    match provenance {
        TokenOrigin::Source { token_range } => source_range_from_trace(&token_range)
            .map(|token_range| SourceTokenProvenanceFact::Source { token_range })
            .unwrap_or(SourceTokenProvenanceFact::Unavailable),
        TokenOrigin::MacroBody { macro_name, identity, call_range, body_token_range } => {
            let Some(call_range) = source_range_from_trace(&call_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            let Some(body_token_range) = source_range_from_trace(&body_token_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            SourceTokenProvenanceFact::MacroBody {
                macro_name: macro_name.to_smolstr(),
                identity: Some(SourceMacroBodyIdentity::from(identity)),
                call_range,
                body_token_range,
            }
        }
        TokenOrigin::MacroArgument {
            macro_name,
            identity,
            call_range,
            body_token_range,
            argument_token_range,
        } => {
            let Some(call_range) = source_range_from_trace(&call_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            let Some(body_token_range) = source_range_from_trace(&body_token_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            let Some(argument_token_range) = source_range_from_trace(&argument_token_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            SourceTokenProvenanceFact::MacroArgument {
                macro_name: macro_name.to_smolstr(),
                identity: Some(SourceMacroArgumentIdentity::from(identity)),
                call_range,
                body_token_range,
                argument_token_range,
            }
        }
        TokenOrigin::Builtin { name, identity } if !name.is_empty() => {
            SourceTokenProvenanceFact::Builtin {
                name: name.to_smolstr(),
                identity: Some(SourceMacroBuiltinIdentity::from(identity)),
            }
        }
        TokenOrigin::TokenPaste { identity } => SourceTokenProvenanceFact::TokenPaste {
            identity: Some(SourceMacroOperationIdentity::from(identity)),
        },
        TokenOrigin::Stringification { identity } => SourceTokenProvenanceFact::Stringification {
            identity: Some(SourceMacroOperationIdentity::from(identity)),
        },
        TokenOrigin::Builtin { .. } => SourceTokenProvenanceFact::Unavailable,
        TokenOrigin::Unavailable => SourceTokenProvenanceFact::Unavailable,
    }
}

fn required_event_range(
    source_order: usize,
    kind: MacroEventKind,
    directive: &Event,
) -> Result<SourceRange, SourcePreprocError> {
    trace_range(&directive.range)
        .ok_or(SourcePreprocError::MissingEventRange { source_order, kind })
}

trait TraceTokenOptionExt {
    fn value(&self) -> Option<SmolStr>;
    fn macro_name(&self) -> Option<SmolStr>;
    fn source_range(&self) -> Option<SourceRange>;
    fn include_target(&self) -> MacroIncludeTarget;
}

impl TraceTokenOptionExt for Option<Token> {
    fn value(&self) -> Option<SmolStr> {
        self.as_ref().map(|token| token.value_text.to_smolstr())
    }

    fn macro_name(&self) -> Option<SmolStr> {
        self.as_ref().map(|token| macro_name(token.value_text.as_str()))
    }

    fn source_range(&self) -> Option<SourceRange> {
        self.as_ref().and_then(|token| trace_range(&token.range))
    }

    fn include_target(&self) -> MacroIncludeTarget {
        self.as_ref()
            .map(|token| include_target_from_raw(token.raw_text.to_smolstr()))
            .unwrap_or_else(|| MacroIncludeTarget::Token { raw: SmolStr::new("") })
    }
}

fn trace_range(range: &Option<SourceBufferRange>) -> Option<SourceRange> {
    range.as_ref().and_then(source_range_from_trace)
}

fn source_range_from_trace(range: &SourceBufferRange) -> Option<SourceRange> {
    Some(SourceRange {
        source: PreprocSourceId::from(range.buffer_id),
        range: TextRange::new(
            TextSize::from(u32::try_from(range.range.start).ok()?),
            TextSize::from(u32::try_from(range.range.end).ok()?),
        ),
    })
}

fn event_kind(kind: SyntaxKind) -> Option<MacroEventKind> {
    match kind {
        SyntaxKind::DEFINE_DIRECTIVE => Some(MacroEventKind::Define),
        SyntaxKind::UNDEF_DIRECTIVE => Some(MacroEventKind::Undef),
        SyntaxKind::INCLUDE_DIRECTIVE => Some(MacroEventKind::Include),
        SyntaxKind::IF_DEF_DIRECTIVE
        | SyntaxKind::IF_N_DEF_DIRECTIVE
        | SyntaxKind::ELS_IF_DIRECTIVE => Some(MacroEventKind::Conditional),
        SyntaxKind::ELSE_DIRECTIVE | SyntaxKind::END_IF_DIRECTIVE => Some(MacroEventKind::Branch),
        SyntaxKind::MACRO_USAGE => Some(MacroEventKind::Usage),
        _ => None,
    }
}

fn trace_conditional_kind(kind: SyntaxKind) -> MacroConditionalKind {
    match kind {
        SyntaxKind::IF_DEF_DIRECTIVE => MacroConditionalKind::IfDef,
        SyntaxKind::IF_N_DEF_DIRECTIVE => MacroConditionalKind::IfNDef,
        SyntaxKind::ELS_IF_DIRECTIVE => MacroConditionalKind::ElsIf,
        SyntaxKind::ELSE_DIRECTIVE => MacroConditionalKind::Else,
        SyntaxKind::END_IF_DIRECTIVE => MacroConditionalKind::EndIf,
        _ => unreachable!(),
    }
}

fn push_source_event_record(
    index: &mut SourcePreprocIndex,
    event_id: SourcePreprocEventId,
    kind: MacroEventKind,
    event_index: usize,
    range: SourceRange,
) {
    index.event_records.push(SourcePreprocEventRecord {
        event_id,
        kind,
        range,
        index: event_index,
    });
}

fn include_target_from_raw(raw: SmolStr) -> MacroIncludeTarget {
    if let Some(path) = strip_include_delimiters(&raw) {
        MacroIncludeTarget::Literal { path: path.to_smolstr(), raw }
    } else {
        MacroIncludeTarget::Token { raw }
    }
}

fn strip_include_delimiters(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let (first, last) = (*bytes.first()?, *bytes.last()?);
    match (first, last) {
        (b'"', b'"') | (b'<', b'>') if raw.len() >= 2 => Some(&raw[1..raw.len() - 1]),
        _ => None,
    }
}

fn macro_name(name: &str) -> SmolStr {
    name.strip_prefix('`').unwrap_or(name).to_smolstr()
}
