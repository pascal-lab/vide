use super::*;

#[test]
fn source_model_resolves_conditional_tokens_to_visible_defines() {
    let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
wire active;
`endif
"#;
    let header_text = "`define HEADER_FLAG\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let conditional_index = model
        .conditionals()
        .iter()
        .position(|conditional| conditional.kind == MacroConditionalKind::IfDef)
        .expect("ifdef should be traced");
    let reference = reference_for_conditional_token(&model, conditional_index, 0);

    assert_eq!(reference.name.as_str(), "HEADER_FLAG");
    assert_eq!(reference.name_range.source, root_source);
    let SourceMacroResolution::Resolved { definition, reason, .. } = reference.resolution else {
        panic!("conditional token reference should resolve to visible definition");
    };
    assert_eq!(reason, SourceMacroResolutionReason::VisibleDefinition);
    assert_eq!(model.macro_definitions().get(definition).unwrap().name_range.source, header_source);
}

#[test]
fn source_model_resolves_ifndef_include_guard_to_following_define() {
    let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
wire active;
`endif
"#;
    let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
    let (model, _root_source, header_source) = source_model(root_text, header_text);

    let conditional_index = model
        .conditionals()
        .iter()
        .position(|conditional| {
            conditional.kind == MacroConditionalKind::IfNDef
                && conditional.range.source == header_source
        })
        .expect("ifndef guard should be traced");
    let reference = model
        .macro_references()
        .iter()
        .find(|reference| {
            matches!(
                reference.site,
                SourceMacroReferenceSite::IncludeGuardIfNDef {
                    conditional_index: site_conditional_index,
                    token_index: 0,
                } if site_conditional_index == conditional_index
            )
        })
        .expect("include guard token should be modeled as a resolved reference");
    assert_eq!(reference.name.as_str(), "HEADER_FLAG");
    assert_eq!(reference.name_range.source, header_source);
    assert!(matches!(
        reference.resolution,
        SourceMacroResolution::Resolved {
            reason: SourceMacroResolutionReason::IncludeGuardIfNDef,
            ..
        }
    ));
}

#[test]
fn source_model_nested_include_resolution_carries_definition_chain() {
    let root_text = r#"`include "defs.vh"
logic [`LEAF_WIDTH-1:0] data;
"#;
    let header_text = "`include \"leaf.vh\"\n";
    let leaf_path = "sample/include/leaf.vh";
    let options = SyntaxTreeOptions {
        include_paths: vec![INCLUDE_DIR.to_owned()],
        include_buffers: vec![
            SyntaxTreeBuffer { path: HEADER_PATH.to_owned(), text: header_text.to_owned() },
            SyntaxTreeBuffer {
                path: leaf_path.to_owned(),
                text: "`define LEAF_WIDTH 4\n".to_owned(),
            },
        ],
        expand_includes: true,
        ..SyntaxTreeOptions::default()
    };
    let trace = preprocessor_trace(root_text, "source", ROOT_PATH, &options);
    let root_source = PreprocSourceId::from(trace.root_buffer_id);
    let model = SourcePreprocModel::from_trace(trace).unwrap();
    let header_source = source_by_path_suffix(&model, "include/defs.vh");
    let leaf_source = source_by_path_suffix(&model, "include/leaf.vh");

    let usage_index = model
        .usages()
        .iter()
        .position(|usage| usage.name.as_deref() == Some("LEAF_WIDTH"))
        .expect("root macro usage should be traced");
    let reference = reference_for_usage(&model, usage_index);
    let SourceMacroResolution::Resolved { definition, include_chain, .. } = &reference.resolution
    else {
        panic!("usage reference should resolve to nested included definition");
    };

    assert_eq!(model.macro_definitions().get(*definition).unwrap().name_range.source, leaf_source);
    assert_eq!(include_chain.len(), 2);
    assert_eq!(include_chain[0].include_range.source, root_source);
    assert_eq!(include_chain[0].included_source, header_source);
    assert_eq!(include_chain[1].include_range.source, header_source);
    assert_eq!(include_chain[1].included_source, leaf_source);
}

#[test]
fn source_model_fails_closed_when_directive_event_range_is_missing() {
    let trace = Trace {
        root_buffer_id: 1,
        source_buffers: vec![SourceBufferId {
            path: ROOT_PATH.to_owned(),
            text: None,
            buffer_id: 1,
            origin: SourceBufferOrigin::Source,
        }],
        events: vec![Event {
            event_id: EventId(0),
            kind: SyntaxKind::DEFINE_DIRECTIVE,
            range: None,
            macro_definition_id: None,
            macro_call_id: None,
            macro_expansion_id: None,
            parent_macro_expansion_id: None,
            directive: None,
            name: Some(Token {
                raw_text: "WIDTH".to_owned(),
                value_text: "WIDTH".to_owned(),
                token_kind: TokenKind::IDENTIFIER,
                range: Some(SourceBufferRange { buffer_id: 1, range: 8..13 }),
            }),
            include_file_name: None,
            params: Vec::new(),
            arguments: Vec::new(),
            body_tokens: Vec::new(),
            expr_tokens: Vec::new(),
            disabled_ranges: Vec::new(),
        }],
        include_edges: Vec::new(),
        emitted_tokens: Vec::new(),
    };

    assert_eq!(
        SourcePreprocModel::from_trace(trace).unwrap_err(),
        SourcePreprocError::MissingEventRange { source_order: 0, kind: MacroEventKind::Define }
    );
}
