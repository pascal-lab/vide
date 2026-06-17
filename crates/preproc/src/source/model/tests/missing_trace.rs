use super::*;

#[test]
fn source_model_marks_missing_direct_trace_partial_without_range_fallback() {
    let root_source = PreprocSourceId::from(1);
    let define_range = source_range(root_source, 0, 11);
    let name_range = source_range(root_source, 8, 9);
    let body_range = source_range(root_source, 10, 11);
    let usage_range = source_range(root_source, 24, 26);
    let index = SourcePreprocIndex {
        root_source: Some(root_source),
        sources: vec![PreprocSource {
            id: root_source,
            path: SmolStr::new(ROOT_PATH),
            origin: PreprocSourceOrigin::Root,
        }],
        event_records: vec![
            SourcePreprocEventRecord {
                event_id: SourcePreprocEventId(0),
                kind: MacroEventKind::Define,
                range: define_range,
                index: 0,
            },
            SourcePreprocEventRecord {
                event_id: SourcePreprocEventId(1),
                kind: MacroEventKind::Usage,
                range: usage_range,
                index: 0,
            },
        ],
        emitted_tokens: vec![SourceEmittedTokenRecord {
            raw: SmolStr::new("1"),
            value: SmolStr::new("1"),
            display: SmolStr::new("1"),
            kind: SourceTokenKind::Syntax(TokenKind::INTEGER_LITERAL),
            origin: TokenOrigin::MacroBody {
                macro_name: "A".to_owned(),
                identity: MacroBodyOrigin {
                    call_id: MacroCallId(20),
                    definition_id: MacroDefinitionId(99),
                    expansion_id: MacroExpansionId(30),
                    parent_expansion_id: None,
                    body_token_index: 0,
                },
                call_range: source_buffer_range(usage_range),
                body_token_range: source_buffer_range(body_range),
            },
        }],
        defines: vec![SourceMacroDefine {
            event_id: SourcePreprocEventId(0),
            trace_definition: Some(MacroDefinitionId(10)),
            name: Some(SmolStr::new("A")),
            name_range: Some(name_range),
            params: None,
            body: vec![SourceMacroToken {
                raw: SmolStr::new("1"),
                value: SmolStr::new("1"),
                range: Some(body_range),
            }],
            range: define_range,
        }],
        usages: vec![SourceMacroUsage {
            event_id: SourcePreprocEventId(1),
            trace_call: Some(MacroCallId(20)),
            trace_definition: None,
            trace_expansion: None,
            parent_trace_expansion: None,
            name: Some(SmolStr::new("A")),
            name_range: Some(usage_range),
            arguments: Vec::new(),
            range: usage_range,
        }],
        ..SourcePreprocIndex::default()
    };

    let model = SourcePreprocModel::new(index);
    let emitted = model.emitted_tokens().iter().next().unwrap();
    assert_eq!(emitted.origin, None);
}

fn source_buffer_range(range: SourceRange) -> SourceBufferRange {
    SourceBufferRange {
        buffer_id: range.source.raw(),
        range: usize::from(range.range.start())..usize::from(range.range.end()),
    }
}
