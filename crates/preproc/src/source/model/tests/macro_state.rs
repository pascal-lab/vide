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
    assert_eq!(
        model.macro_definitions().get(after_include.id).unwrap().name_range.source,
        header_source
    );

    assert!(
        visible_macro_definition(
            &model,
            root_source,
            offset_after(root_text, "`undef HEADER_WIDTH\n"),
            "HEADER_WIDTH",
        )
        .is_none()
    );
}

#[test]
fn source_model_same_name_define_overrides_included_define() {
    let root_text = r#"`include "defs.vh"
`define HEADER_WIDTH 16
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let after_override = visible_macro_definition(
        &model,
        root_source,
        offset_after(root_text, "`define HEADER_WIDTH 16\n"),
        "HEADER_WIDTH",
    )
    .unwrap();
    assert_eq!(after_override.id.raw(), 1);

    let definitions = model
        .macro_definitions()
        .iter()
        .filter(|definition| definition.name == "HEADER_WIDTH")
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 2);
    let header_definition =
        definitions.iter().find(|definition| definition.name_range.source == header_source).unwrap();
    assert_eq!(header_definition.body_tokens[0].value.as_str(), "8");
    let root_definition =
        definitions.iter().find(|definition| definition.name_range.source == root_source).unwrap();
    assert_eq!(root_definition.body_tokens[0].value.as_str(), "16");

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

    let reference = model
        .macro_references()
        .iter()
        .find(|reference| {
            matches!(reference.site, SourceMacroReferenceSite::Usage { .. })
                && reference.name == "HEADER_WIDTH"
                && reference.directive_range.source == root_source
        })
        .expect("root macro usage should be traced");
    assert_eq!(reference.name_range.source, root_source);

    let SourceMacroResolution::Resolved { definition } = &reference.resolution else {
        panic!("usage reference should resolve to included definition");
    };
    let definition = model.macro_definitions().get(*definition).unwrap();
    assert_eq!(definition.name.as_str(), "HEADER_WIDTH");
    assert_eq!(definition.name_range.source, header_source);
    assert_eq!(definition.body_tokens[0].value.as_str(), "8");
}
