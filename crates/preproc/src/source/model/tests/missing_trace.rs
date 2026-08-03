use super::*;

#[test]
fn source_model_marks_missing_direct_trace_partial_without_range_fallback() {
    let root_source = PreprocSourceId::from(1);
    let define_range = source_range(root_source, 0, 11);
    let name_range = source_range(root_source, 8, 9);
    let body_range = source_range(root_source, 10, 11);
    let usage_range = source_range(root_source, 24, 26);
    let trace = Trace {
        root_buffer_id: 1,
        source_buffers: vec![SourceBufferId {
            path: ROOT_PATH.to_owned(),
            text: None,
            buffer_id: 1,
            origin: SourceBufferOrigin::Source,
        }],
        events: vec![
            Event {
                event_id: EventId(0),
                kind: SyntaxKind::DEFINE_DIRECTIVE,
                range: Some(source_buffer_range(define_range)),
                macro_definition_id: Some(MacroDefinitionId(10)),
                macro_call_id: None,
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(Token {
                    raw_text: "A".to_owned(),
                    value_text: "A".to_owned(),
                    token_kind: TokenKind::IDENTIFIER,
                    range: Some(source_buffer_range(name_range)),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: vec![Token {
                    raw_text: "1".to_owned(),
                    value_text: "1".to_owned(),
                    token_kind: TokenKind::INTEGER_LITERAL,
                    range: Some(source_buffer_range(body_range)),
                }],
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
            Event {
                event_id: EventId(1),
                kind: SyntaxKind::MACRO_USAGE,
                range: Some(source_buffer_range(usage_range)),
                macro_definition_id: None,
                macro_call_id: Some(MacroCallId(20)),
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(Token {
                    raw_text: "`A".to_owned(),
                    value_text: "`A".to_owned(),
                    token_kind: TokenKind::DIRECTIVE,
                    range: Some(source_buffer_range(usage_range)),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: Vec::new(),
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
        ],
        include_edges: Vec::new(),
        emitted_tokens: vec![syntax::EmittedToken {
            emitted_token_index: None,
            raw_text: "1".to_owned(),
            value_text: "1".to_owned(),
            display_text: "1".to_owned(),
            token_kind: TokenKind::INTEGER_LITERAL,
            origin: TokenOrigin::MacroBody {
                macro_name: "A".to_owned(),
                call_id: MacroCallId(20),
                definition_id: MacroDefinitionId(99),
                expansion_id: MacroExpansionId(30),
                parent_expansion_id: None,
                body_token_index: 0,
                call_range: source_buffer_range(usage_range),
                body_token_range: source_buffer_range(body_range),
            },
        }],
    };

    let model = SourcePreprocModel::from_trace(trace).unwrap();
    let emitted = model.emitted_tokens().iter().next().unwrap();
    assert_eq!(emitted.origin, None);
}

fn source_buffer_range(range: SourceRange) -> SourceBufferRange {
    SourceBufferRange {
        buffer_id: range.source.raw(),
        range: usize::from(range.range.start())..usize::from(range.range.end()),
    }
}
