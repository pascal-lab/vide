use super::*;

#[test]
fn source_model_applies_include_define_after_include_point_only() {
    let root_text = r#"`include "defs.vh"
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    assert!(
        !visible_macro_names(&model, root_source, offset_before(root_text, "`include"))
            .iter()
            .any(|name| name == "HEADER_WIDTH")
    );

    let after_include = visible_macro_definition(
        &model,
        root_source,
        offset_after(root_text, "`include \"defs.vh\"\n"),
        "HEADER_WIDTH",
    )
    .unwrap();
    assert_eq!(after_include.id.raw(), 0);

    let definition = model
        .visible_macros_at(SourcePosition {
            source: root_source,
            offset: offset_after(root_text, "`include \"defs.vh\"\n"),
        })
        .into_iter()
        .find(|definition| definition.name == "HEADER_WIDTH")
        .unwrap();
    assert_eq!(definition.name_range.source, header_source);
}

#[test]
fn source_model_undef_removes_included_define() {
    let root_text = r#"`include "defs.vh"
`undef HEADER_WIDTH
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let after_include = visible_macro_definition(
        &model,
        root_source,
        offset_after(root_text, "`include \"defs.vh\"\n"),
        "HEADER_WIDTH",
    )
    .unwrap();
    assert_eq!(after_include.id.raw(), 0);
    assert_eq!(model.defines()[0].name_range.unwrap().source, header_source);

    assert!(
        visible_macro_definition(
            &model,
            root_source,
            offset_after(root_text, "`undef HEADER_WIDTH\n"),
            "HEADER_WIDTH",
        )
        .is_none()
    );
    assert_eq!(model.undefs()[0].name.as_deref(), Some("HEADER_WIDTH"));
    assert_eq!(model.undefs()[0].name_range.unwrap().source, root_source);
}

#[test]
fn source_model_same_name_define_overrides_included_define() {
    let root_text = r#"`include "defs.vh"
`define HEADER_WIDTH 16
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    assert_eq!(model.defines()[0].name_range.unwrap().source, header_source);
    assert_eq!(model.defines()[1].name_range.unwrap().source, root_source);

    let after_override = visible_macro_definition(
        &model,
        root_source,
        offset_after(root_text, "`define HEADER_WIDTH 16\n"),
        "HEADER_WIDTH",
    )
    .unwrap();
    assert_eq!(after_override.id.raw(), 1);

    let definition = model
        .visible_macros_at(SourcePosition {
            source: root_source,
            offset: offset_after(root_text, "`define HEADER_WIDTH 16\n"),
        })
        .into_iter()
        .find(|definition| definition.name == "HEADER_WIDTH")
        .unwrap();
    assert_eq!(definition.body_tokens[0].value.as_str(), "16");
    assert_eq!(definition.name_range.source, root_source);
}

#[test]
fn visible_macro_query_reads_timeline_without_event_records() {
    let root_text = r#"`define A 1
`undef A
`define B 2
"#;
    let trace = preprocessor_trace(root_text, "source", ROOT_PATH, &SyntaxTreeOptions::default());
    let root_source = PreprocSourceId::from(trace.root_buffer_id);
    let mut model = SourcePreprocModel::from_trace(trace).unwrap();

    let names_after_define =
        visible_macro_names(&model, root_source, offset_after(root_text, "`define A 1\n"));
    let names_after_undef =
        visible_macro_names(&model, root_source, offset_after(root_text, "`undef A\n"));
    let names_after_second_define =
        visible_macro_names(&model, root_source, offset_after(root_text, "`define B 2\n"));

    assert_eq!(names_after_define, vec![SmolStr::new("A")]);
    assert!(names_after_undef.is_empty(), "{names_after_undef:?}");
    assert_eq!(names_after_second_define, vec![SmolStr::new("B")]);

    model.index.event_records.clear();

    assert_eq!(
        visible_macro_names(&model, root_source, offset_after(root_text, "`define A 1\n")),
        names_after_define
    );
    assert_eq!(
        visible_macro_names(&model, root_source, offset_after(root_text, "`undef A\n")),
        names_after_undef
    );
    assert_eq!(
        visible_macro_names(&model, root_source, offset_after(root_text, "`define B 2\n")),
        names_after_second_define
    );
}

#[test]
fn included_plain_source_uses_include_scope_macro_state() {
    let root_text = r#"`define BEFORE 1
`include "defs.vh"
`define AFTER 1
"#;
    let header_text = "wire x;\n";
    let (model, _, header_source) = source_model(root_text, header_text);

    let names = visible_macro_names(&model, header_source, offset_after(header_text, "wire x"));

    assert!(names.iter().any(|name| name == "BEFORE"), "{names:?}");
    assert!(!names.iter().any(|name| name == "AFTER"), "{names:?}");
}

#[test]
fn included_source_after_last_directive_uses_include_scope_macro_state() {
    let root_text = r#"`define BEFORE 1
`include "defs.vh"
`define AFTER 1
"#;
    let header_text = "`define FROM_HEADER 1\nwire x;\n";
    let (model, _, header_source) = source_model(root_text, header_text);

    let names = visible_macro_names(&model, header_source, offset_after(header_text, "wire x"));

    assert!(names.iter().any(|name| name == "BEFORE"), "{names:?}");
    assert!(names.iter().any(|name| name == "FROM_HEADER"), "{names:?}");
    assert!(!names.iter().any(|name| name == "AFTER"), "{names:?}");
}

#[test]
fn source_model_preserves_inactive_range_sources() {
    let root_text = r#"`include "defs.vh"
`ifndef HEADER_FLAG
wire disabled_by_header;
`endif
"#;
    let header_text = r#"`define HEADER_FLAG
`ifdef NEVER
wire disabled_from_header;
`endif
"#;
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let root_inactive =
        model.inactive_ranges().iter().find(|range| range.source == root_source).unwrap();
    assert_eq!(text_at_range(root_text, root_inactive.range), "wire disabled_by_header;");

    let header_inactive =
        model.inactive_ranges().iter().find(|range| range.source == header_source).unwrap();
    assert_eq!(text_at_range(header_text, header_inactive.range), "wire disabled_from_header;");
}

#[test]
fn source_model_resolves_root_usage_to_included_define() {
    let root_text = r#"`include "defs.vh"
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let usage_index = model
        .usages()
        .iter()
        .position(|usage| usage.name.as_deref() == Some("HEADER_WIDTH"))
        .expect("root macro usage should be traced");
    let usage = &model.usages()[usage_index];
    assert_eq!(usage.range.source, root_source);
    assert_eq!(usage.name_range.unwrap().source, root_source);

    let reference = reference_for_usage(&model, usage_index);
    let SourceMacroResolution::Resolved { definition, include_chain, reason } =
        &reference.resolution
    else {
        panic!("usage reference should resolve to included definition");
    };
    assert_eq!(*reason, SourceMacroResolutionReason::VisibleDefinition);
    let definition = model.macro_definitions().get(*definition).unwrap();
    assert_eq!(definition.name.as_str(), "HEADER_WIDTH");
    assert_eq!(definition.name_range.source, header_source);
    assert_eq!(definition.body_tokens[0].value.as_str(), "8");
    assert_eq!(include_chain.len(), 1);
    assert_eq!(include_chain[0].include_range.source, root_source);
    assert_eq!(include_chain[0].included_source, header_source);
}
