use super::*;

#[test]
fn preproc_included_define_references_include_root_conditionals() {
    let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
localparam int ENABLED = `HEADER_FLAG;
`endif
"#;
    let header_text = "`define HEADER_FLAG 1\n";
    let db = db_with_files(root_text, header_text);
    let definition =
        macro_definition_at(&db, HEADER, offset_after(header_text, "`define ")).unwrap().unwrap();

    assert_eq!(definition.file_id, HEADER);

    let refs = macro_references(&db, HEADER, &definition).unwrap().references;

    assert!(refs.iter().any(|reference| {
        reference.file_id == TOP && text_at_range(root_text, reference.range) == "HEADER_FLAG"
    }));
    let definitions =
        macro_reference_definitions_at(&db, TOP, offset_after(root_text, "ENABLED = `"))
            .unwrap()
            .unwrap();
    assert_eq!(text_at_range(root_text, definitions.range), "`HEADER_FLAG");
    assert!(macro_reference_definitions_at(&db, TOP, definitions.range.end()).unwrap().is_none());
    assert!(macro_usage_resolution_at(&db, TOP, definitions.range.end()).unwrap().is_none());
    assert!(definitions.definitions.iter().any(|indexed| {
        indexed.file_id == HEADER
            && indexed.name_range == definition.name_range
            && indexed.name == definition.name
    }));
}

#[test]
fn preproc_header_ifdef_reference_uses_including_root_context() {
    let root_text = r#"`include "defs.vh"
`include "leaf.vh"
"#;
    let header_text = "`define FEATURE_B 1\n";
    let leaf_text = r#"`ifdef FEATURE_B
wire enabled;
`endif
"#;
    let db = db_with_nested_files(root_text, header_text, leaf_text);

    let definitions =
        macro_reference_definitions_at(&db, LEAF, offset(leaf_text, "FEATURE_B")).unwrap().unwrap();

    assert_eq!(text_at_range(leaf_text, definitions.range), "FEATURE_B");
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == HEADER
            && text_at_range(header_text, definition.name_range) == "FEATURE_B"
    }));
}

#[test]
fn preproc_header_macro_body_references_use_expansion_context() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `DEMO_WIDTH;
localparam int N = `DEMO_NEXT(1);
localparam int R = `DEMO_RESET;
endmodule
"#;
    let header_text = r#"`ifndef SHARED_DEFS_SVH
`define SHARED_DEFS_SVH
`include "leaf.vh"
`define DEMO_WIDTH `MATH_WIDTH
`define DEMO_RESET {`DEMO_WIDTH{1'b0}}
`define DEMO_NEXT(value) ((value) + `MATH_ONE)
`endif
"#;
    let leaf_text = r#"`define MATH_WIDTH 12
`define MATH_ONE 12'd1
"#;
    let db = db_with_nested_files(root_text, header_text, leaf_text);

    let math_width = macro_reference_definitions_at(&db, HEADER, offset(header_text, "MATH_WIDTH"))
        .unwrap()
        .unwrap();
    assert!(math_width.definitions.iter().any(|definition| {
        definition.file_id == LEAF
            && text_at_range(leaf_text, definition.name_range) == "MATH_WIDTH"
    }));

    let math_one = macro_reference_definitions_at(&db, HEADER, offset(header_text, "MATH_ONE"))
        .unwrap()
        .unwrap();
    assert!(math_one.definitions.iter().any(|definition| {
        definition.file_id == LEAF && text_at_range(leaf_text, definition.name_range) == "MATH_ONE"
    }));

    let demo_width = macro_reference_definitions_at(
        &db,
        HEADER,
        offset_after(header_text, "`define DEMO_RESET {`"),
    )
    .unwrap()
    .unwrap();
    assert!(demo_width.definitions.iter().any(|definition| {
        definition.file_id == HEADER
            && text_at_range(header_text, definition.name_range) == "DEMO_WIDTH"
    }));
}

#[test]
fn preproc_macro_param_references_resolve_to_formals() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `SHIFT(4, 1);
endmodule
"#;
    let header_text = "`define SHIFT(value, amount) ((value) << amount)\n";
    let db = db_with_files(root_text, header_text);

    let value_definition =
        macro_param_definition_at(&db, HEADER, offset_after(header_text, "SHIFT("))
            .unwrap()
            .unwrap();
    assert_eq!(value_definition.name.as_str(), "value");
    assert_eq!(text_at_range(header_text, value_definition.range), "value");
    assert!(
        macro_param_definition_at(&db, HEADER, value_definition.range.end()).unwrap().is_none()
    );

    let value_reference = macro_param_reference_definitions_at(
        &db,
        HEADER,
        offset_after(header_text, "SHIFT(value, amount) (("),
    )
    .unwrap()
    .unwrap();
    assert_eq!(text_at_range(header_text, value_reference.range), "value");
    assert!(
        macro_param_reference_definitions_at(&db, HEADER, value_reference.range.end())
            .unwrap()
            .is_none()
    );
    assert!(value_reference.definitions.iter().any(|definition| {
        definition.param_index == value_definition.param_index
            && text_at_range(header_text, definition.range) == "value"
    }));

    let refs = macro_param_references(&db, HEADER, &value_definition).unwrap().references;
    assert!(refs.iter().any(|reference| {
        reference.file_id == HEADER && text_at_range(header_text, reference.range) == "value"
    }));
    assert!(!refs.iter().any(|reference| text_at_range(header_text, reference.range) == "amount"));
}
