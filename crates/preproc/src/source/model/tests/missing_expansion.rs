use super::*;

#[test]
fn source_model_does_not_create_expansion_without_emitted_tokens() {
    let root_text = "`define A 1\nmodule m; localparam int W = `A; endmodule\n";
    let define_start = root_text.find("`define").unwrap();
    let define_end = root_text.find('\n').unwrap();
    let usage_start = root_text.find("`A").unwrap();
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
                range: Some(SourceBufferRange { buffer_id: 1, range: define_start..define_end }),
                macro_definition_id: None,
                macro_call_id: None,
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(Token {
                    raw_text: "A".to_owned(),
                    value_text: "A".to_owned(),
                    token_kind: TokenKind::IDENTIFIER,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..9 }),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: vec![Token {
                    raw_text: "1".to_owned(),
                    value_text: "1".to_owned(),
                    token_kind: TokenKind::INTEGER_LITERAL,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 10..11 }),
                }],
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
            Event {
                event_id: EventId(1),
                kind: SyntaxKind::MACRO_USAGE,
                range: Some(SourceBufferRange {
                    buffer_id: 1,
                    range: usage_start..usage_start + 2,
                }),
                macro_definition_id: None,
                macro_call_id: None,
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(Token {
                    raw_text: "`A".to_owned(),
                    value_text: "`A".to_owned(),
                    token_kind: TokenKind::DIRECTIVE,
                    range: Some(SourceBufferRange {
                        buffer_id: 1,
                        range: usage_start..usage_start + 2,
                    }),
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
        emitted_tokens: Vec::new(),
    };
    let model = SourcePreprocModel::from_trace(trace).unwrap();
    let call = model.macro_calls().iter().next().expect("usage should create a call");

    assert!(model.macro_expansions().is_empty());
    assert!(matches!(
        model.immediate_macro_expansion(call.id),
        Err(SourcePreprocUnavailable::MissingMacroExpansion { .. })
    ));
}

#[test]
fn source_model_keeps_zero_token_macro_expansion_available() {
    let root_text = r#"`define EMPTY
`define DROP(x)
module top;
`EMPTY
`DROP(foo)
endmodule
"#;
    let (model, _root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    for name in ["EMPTY", "DROP"] {
        let call = model
            .macro_calls()
            .iter()
            .find(|call| {
                model
                    .macro_references()
                    .get(call.reference)
                    .is_some_and(|reference| reference.name.as_str() == name)
            })
            .unwrap_or_else(|| panic!("{name} call should be traced"));
        let Ok(expansion_id) = model.immediate_macro_expansion(call.id) else {
            panic!("{name} zero-token expansion should be available: {call:?}");
        };
        let expansion = model.macro_expansions().get(expansion_id).unwrap();
        assert_eq!(expansion.emitted_token_range.len, 0);
        assert_eq!(expansion.call, call.id);
    }
}
