use std::collections::BTreeMap;

use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SyntaxKind,
    preproc::{
        ActualArgument, Event, MacroParam, SourceBufferOrigin, SourceBufferRange, Token, Trace,
    },
};
use utils::line_index::{TextRange, TextSize};

use super::*;

impl SourcePreprocModelBuilder {
    /// Collect the raw event projections from the preprocessor trace into the
    /// builder's private fields, ready for table derivation.
    pub(in crate::source) fn collect(trace: &Trace) -> Result<Self, SourcePreprocError> {
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
        let sources = trace
            .source_buffers
            .iter()
            .map(|source| PreprocSource {
                id: PreprocSourceId::from(source.buffer_id),
                path: source.path.to_smolstr(),
                origin: source_origin(
                    PreprocSourceId::from(source.buffer_id),
                    root_source,
                    source.origin,
                    &included_by,
                ),
            })
            .collect::<Vec<_>>();

        if !sources.iter().any(|source| source.id == root_source) {
            return Err(SourcePreprocError::MissingRootSource);
        }

        let mut builder = Self {
            model: SourcePreprocModel {
                root_source,
                sources,
                inactive_ranges: Vec::new(),
                macro_definitions: SourceMacroDefinitionTable::default(),
                macro_references: SourceMacroReferenceTable::default(),
                macro_calls: SourceMacroCallTable::default(),
                include_graph: SourceIncludeGraph::default(),
                state_timeline: SourceMacroStateTimeline::default(),
            },
            event_records: Vec::new(),
            defines: Vec::new(),
            undefs: Vec::new(),
            includes: Vec::new(),
            conditionals: Vec::new(),
            usages: Vec::new(),
            include_edges,
            definition_ids_by_define_index: BTreeMap::new(),
            definitions_by_trace_id: BTreeMap::new(),
            calls_by_trace_id: BTreeMap::new(),
            current_state: BTreeMap::new(),
        };

        for (source_order, directive) in trace.events.iter().enumerate() {
            builder.collect_trace_event(source_order, directive)?;
        }

        Ok(builder)
    }

    fn collect_trace_event(
        &mut self,
        source_order: usize,
        directive: &Event,
    ) -> Result<(), SourcePreprocError> {
        self.model.inactive_ranges.extend(
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
                let event_index = self.defines.len();
                let define = collect_trace_define(directive.clone(), event_id, range);
                self.defines.push(define);
                self.push_source_event_record(event_id, kind, event_index, range);
            }
            MacroEventKind::Undef => {
                let event_index = self.undefs.len();
                self.undefs.push(SourceMacroUndef {
                    event_id,
                    name: directive.name.value(),
                    name_range: directive.name.source_range(),
                    range,
                });
                self.push_source_event_record(event_id, kind, event_index, range);
            }
            MacroEventKind::Include => {
                let event_index = self.includes.len();
                let target = directive.include_file_name.include_target();
                self.includes.push(SourceMacroInclude {
                    event_id,
                    target,
                    target_range: directive.include_file_name.source_range(),
                    range,
                });
                self.push_source_event_record(event_id, kind, event_index, range);
            }
            MacroEventKind::Conditional | MacroEventKind::Branch => {
                let event_index = self.conditionals.len();
                self.conditionals.push(SourceMacroConditional {
                    event_id,
                    kind: trace_conditional_kind(directive.kind),
                    expr: directive
                        .expr_tokens
                        .iter()
                        .cloned()
                        .map(macro_token_from_trace)
                        .collect(),
                    range,
                });
                self.push_source_event_record(event_id, kind, event_index, range);
            }
            MacroEventKind::Usage => {
                let event_index = self.usages.len();
                self.usages.push(SourceMacroUsage {
                    event_id,
                    trace_call: directive.macro_call_id,
                    trace_definition: directive.macro_definition_id,
                    name: directive.name.macro_name(),
                    name_range: directive.name.source_range(),
                    arguments: directive
                        .arguments
                        .iter()
                        .cloned()
                        .enumerate()
                        .map(macro_actual_argument_from_trace)
                        .collect(),
                    range,
                });
                self.push_source_event_record(event_id, kind, event_index, range);
            }
        }

        Ok(())
    }

    fn push_source_event_record(
        &mut self,
        event_id: SourcePreprocEventId,
        kind: MacroEventKind,
        event_index: usize,
        range: SourceRange,
    ) {
        self.event_records.push(SourcePreprocEventRecord {
            event_id,
            kind,
            range,
            index: event_index,
        });
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

fn collect_trace_define(
    directive: Event,
    event_id: SourcePreprocEventId,
    range: SourceRange,
) -> SourceMacroDefine {
    SourceMacroDefine {
        event_id,
        trace_definition: directive.macro_definition_id,
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

fn include_target_from_raw(raw: SmolStr) -> MacroIncludeTarget {
    if let Some(path) = strip_include_delimiters(&raw) {
        MacroIncludeTarget::Literal { path: path.to_smolstr() }
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
