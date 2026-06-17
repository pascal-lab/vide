use super::*;

#[test]
fn source_model_marks_missing_direct_identity_partial_without_range_fallback() {
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
        emitted_tokens: vec![SourceEmittedTokenFact {
            raw: SmolStr::new("1"),
            value: SmolStr::new("1"),
            display: SmolStr::new("1"),
            kind: SourceTokenKind::Syntax(TokenKind::INTEGER_LITERAL),
            provenance: SourceTokenProvenanceFact::MacroBody {
                macro_name: SmolStr::new("A"),
                identity: None,
                call_range: usage_range,
                body_token_range: body_range,
            },
        }],
        defines: vec![SourceMacroDefine {
            event_id: SourcePreprocEventId(0),
            identity: Some(SourceMacroDefinitionKey::new(10)),
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
            identity: Some(SourceMacroCallKey::new(20)),
            definition_identity: None,
            expansion_identity: None,
            parent_expansion_identity: None,
            name: Some(SmolStr::new("A")),
            name_range: Some(usage_range),
            arguments: Vec::new(),
            range: usage_range,
        }],
        ..SourcePreprocIndex::default()
    };

    let model = SourcePreprocModel::new(index);
    let emitted = model.emitted_tokens().iter().next().unwrap();
    assert!(matches!(
        model.token_provenance().get(emitted.provenance).unwrap(),
        SourceTokenProvenance::Unavailable(())
    ));
}
