use std::collections::BTreeMap;

use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    PreprocessorTrace, PreprocessorTraceEvent, PreprocessorTraceEventId,
    PreprocessorTraceMacroParam, PreprocessorTraceToken, SourceBufferOrigin, SourceBufferRange,
    SyntaxKind,
};
use utils::line_index::{TextRange, TextSize};

use super::*;

impl From<PreprocessorTraceEventId> for SourcePreprocEventId {
    fn from(value: PreprocessorTraceEventId) -> Self {
        Self(value.0)
    }
}

impl SourcePreprocIndex {
    pub fn from_trace(trace: PreprocessorTrace) -> Result<Self, SourcePreprocError> {
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

        validate_include_edges(&index)?;

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

fn validate_include_edges(index: &SourcePreprocIndex) -> Result<(), SourcePreprocError> {
    for edge in &index.include_edges {
        if !index.sources.iter().any(|source| source.id == edge.included_source) {
            return Err(SourcePreprocError::MissingIncludedSource {
                include_event_id: edge.include_event_id.raw(),
                source: edge.included_source.raw(),
            });
        }

        let Some(directive) = index
            .event_records
            .iter()
            .find(|directive| directive.event_id == edge.include_event_id)
        else {
            return Err(SourcePreprocError::MissingIncludeEvent {
                include_event_id: edge.include_event_id.raw(),
            });
        };

        if directive.kind != MacroEventKind::Include {
            return Err(SourcePreprocError::IncludeEdgeNotInclude {
                include_event_id: edge.include_event_id.raw(),
            });
        }
    }

    Ok(())
}

fn collect_trace_event(
    index: &mut SourcePreprocIndex,
    source_order: usize,
    directive: PreprocessorTraceEvent,
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
                name: directive.name.as_ref().map(trace_token_value),
                name_range: directive.name.as_ref().and_then(trace_token_range),
                range,
            });
            push_source_event_record(index, event_id, kind, event_index, range);
        }
        MacroEventKind::Include => {
            let event_index = index.includes.len();
            let target = directive
                .include_file_name
                .as_ref()
                .map(|token| include_target_from_raw(token.raw_text.to_smolstr()))
                .unwrap_or_else(|| MacroIncludeTarget::Token { raw: SmolStr::new("") });
            index.includes.push(SourceMacroInclude {
                event_id,
                target,
                target_range: directive.include_file_name.as_ref().and_then(trace_token_range),
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
                name: directive.name.as_ref().map(|token| macro_name(token.value_text.as_str())),
                name_range: directive.name.as_ref().and_then(trace_token_range),
                range,
            });
            push_source_event_record(index, event_id, kind, event_index, range);
        }
    }

    Ok(())
}

fn collect_trace_define(
    directive: PreprocessorTraceEvent,
    event_id: SourcePreprocEventId,
    range: SourceRange,
) -> SourceMacroDefine {
    SourceMacroDefine {
        event_id,
        name: directive.name.as_ref().map(trace_token_value),
        name_range: directive.name.as_ref().and_then(trace_token_range),
        params: (!directive.params.is_empty())
            .then(|| directive.params.into_iter().map(macro_param_from_trace).collect()),
        body: directive.body_tokens.into_iter().map(macro_token_from_trace).collect(),
        range,
    }
}

fn macro_param_from_trace(param: PreprocessorTraceMacroParam) -> SourceMacroParam {
    SourceMacroParam {
        name: param.name.as_ref().map(trace_token_value),
        name_range: param.name.as_ref().and_then(trace_token_range),
        default: param
            .default_tokens
            .map(|tokens| tokens.into_iter().map(macro_token_from_trace).collect()),
        range: param.range.as_ref().and_then(source_range_from_trace),
    }
}

fn macro_token_from_trace(token: PreprocessorTraceToken) -> SourceMacroToken {
    SourceMacroToken {
        raw: token.raw_text.to_smolstr(),
        value: token.value_text.to_smolstr(),
        range: token.range.as_ref().and_then(source_range_from_trace),
    }
}

fn trace_token_value(token: &PreprocessorTraceToken) -> SmolStr {
    token.value_text.to_smolstr()
}

fn trace_token_range(token: &PreprocessorTraceToken) -> Option<SourceRange> {
    token.range.as_ref().and_then(source_range_from_trace)
}

fn required_event_range(
    source_order: usize,
    kind: MacroEventKind,
    directive: &PreprocessorTraceEvent,
) -> Result<SourceRange, SourcePreprocError> {
    directive
        .range
        .as_ref()
        .and_then(source_range_from_trace)
        .ok_or(SourcePreprocError::MissingEventRange { source_order, kind })
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
