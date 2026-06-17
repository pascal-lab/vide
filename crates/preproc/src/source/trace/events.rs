use super::{helpers::*, tokens::emitted_token_from_trace, *};

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
                identity: directive.macro_call_id,
                definition_identity: directive.macro_definition_id,
                expansion_identity: directive.macro_expansion_id,
                parent_expansion_identity: directive.parent_macro_expansion_id,
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
        identity: directive.macro_definition_id,
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
