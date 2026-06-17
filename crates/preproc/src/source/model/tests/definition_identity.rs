use super::*;

#[test]
fn source_model_uses_direct_definition_identity_when_body_ranges_collide() {
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
                range: Some(SourceBufferRange { buffer_id: 1, range: 0..12 }),
                macro_definition_id: Some(MacroDefinitionId(10)),
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
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..9 }),
                }],
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
            Event {
                event_id: EventId(1),
                kind: SyntaxKind::DEFINE_DIRECTIVE,
                range: Some(SourceBufferRange { buffer_id: 1, range: 13..25 }),
                macro_definition_id: Some(MacroDefinitionId(20)),
                macro_call_id: None,
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(Token {
                    raw_text: "B".to_owned(),
                    value_text: "B".to_owned(),
                    token_kind: TokenKind::IDENTIFIER,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 21..22 }),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: vec![Token {
                    raw_text: "2".to_owned(),
                    value_text: "2".to_owned(),
                    token_kind: TokenKind::INTEGER_LITERAL,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..9 }),
                }],
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
            Event {
                event_id: EventId(2),
                kind: SyntaxKind::MACRO_USAGE,
                range: Some(SourceBufferRange { buffer_id: 1, range: 40..42 }),
                macro_definition_id: None,
                macro_call_id: Some(MacroCallId(200)),
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(Token {
                    raw_text: "`B".to_owned(),
                    value_text: "`B".to_owned(),
                    token_kind: TokenKind::DIRECTIVE,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 40..42 }),
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
            raw_text: "2".to_owned(),
            value_text: "2".to_owned(),
            display_text: "2".to_owned(),
            token_kind: TokenKind::INTEGER_LITERAL,
            provenance: TokenOrigin::MacroBody {
                macro_name: "B".to_owned(),
                identity: MacroBodyOrigin {
                    call_id: MacroCallId(200),
                    definition_id: MacroDefinitionId(20),
                    expansion_id: MacroExpansionId(300),
                    parent_expansion_id: None,
                    body_token_index: 0,
                },
                call_range: SourceBufferRange { buffer_id: 1, range: 40..42 },
                body_token_range: SourceBufferRange { buffer_id: 1, range: 8..9 },
            },
        }],
    };
    let model = SourcePreprocModel::from_trace(trace).unwrap();
    let emitted = model.emitted_tokens().iter().find(|token| token.text == "2").unwrap();
    let SourceTokenOrigin::MacroBody { definition, call, identity, .. } =
        model.token_origins().get(emitted.origin.unwrap()).unwrap()
    else {
        panic!("colliding range token should still resolve through direct body identity");
    };

    let definition = model.macro_definitions().get(*definition).unwrap();
    assert_eq!(definition.name.as_str(), "B");
    assert_eq!(definition.identity, Some(identity.definition));
    assert_eq!(model.macro_calls().get(*call).unwrap().identity, Some(identity.call));
}
