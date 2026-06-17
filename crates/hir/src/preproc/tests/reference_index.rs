use super::*;

#[test]
fn preproc_header_reference_reports_all_including_context_definitions() {
    let root_text = r#"`define WIDTH 8
`include "defs.vh"
`undef WIDTH
`define WIDTH 16
`include "defs.vh"
"#;
    let header_text = "localparam int W = `WIDTH;\n";
    let db = db_with_files(root_text, header_text);

    let definitions =
        macro_reference_definitions_at(&db, HEADER, offset(header_text, "WIDTH")).unwrap().unwrap();

    assert_eq!(text_at_range(header_text, definitions.range), "`WIDTH");
    assert_eq!(definitions.definitions.len(), 2);
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP
            && definition.name_range.start() == offset_after_n(root_text, "`define ", 0)
    }));
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP
            && definition.name_range.start() == offset_after_n(root_text, "`define ", 1)
    }));
}

#[test]
fn preproc_header_macro_body_reference_reports_all_expansion_context_definitions() {
    let root_text = r#"`define WIDTH 8
`include "defs.vh"
localparam int A = `USE_WIDTH;
`undef WIDTH
`define WIDTH 16
`include "defs.vh"
localparam int B = `USE_WIDTH;
"#;
    let header_text = "`define USE_WIDTH `WIDTH\n";
    let db = db_with_files(root_text, header_text);

    let definitions =
        macro_reference_definitions_at(&db, HEADER, offset_after(header_text, "USE_WIDTH `"))
            .unwrap()
            .unwrap();

    assert_eq!(text_at_range(header_text, definitions.range), "`WIDTH");
    assert_eq!(definitions.definitions.len(), 2);
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP
            && definition.name_range.start() == offset_after_n(root_text, "`define ", 0)
    }));
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP
            && definition.name_range.start() == offset_after_n(root_text, "`define ", 1)
    }));
}

#[test]
fn preproc_macro_definition_at_only_hits_name_range() {
    let root_text = "`define HEADER_FLAG 1\n";
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    assert!(macro_definition_at(&db, TOP, offset(root_text, "`define")).unwrap().is_none());

    let definition =
        macro_definition_at(&db, TOP, offset(root_text, "HEADER_FLAG")).unwrap().unwrap();
    assert_eq!(text_at_range(root_text, definition.name_range), "HEADER_FLAG");
    assert!(macro_definition_at(&db, TOP, definition.name_range.end()).unwrap().is_none());
    assert_ne!(definition.directive_range, definition.name_range);
}

#[test]
fn preproc_ifndef_guard_reference_resolves_to_following_define() {
    let root_text = "`include \"defs.vh\"\n";
    let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
    let db = db_with_files(root_text, header_text);
    let resolution =
        macro_reference_definitions_at(&db, HEADER, offset(header_text, "HEADER_FLAG"))
            .unwrap()
            .unwrap();

    assert!(resolution.references.iter().any(|reference| reference.file_id == HEADER));
    let definition =
        resolution.definitions.iter().find(|definition| definition.file_id == HEADER).unwrap();
    assert_eq!(text_at_range(header_text, definition.name_range), "HEADER_FLAG");

    let refs = macro_references(&db, HEADER, definition).unwrap().references;
    assert!(refs.iter().any(|reference| {
        reference.file_id == HEADER
            && reference.range.start() == offset(header_text, "HEADER_FLAG")
            && text_at_range(header_text, reference.range) == "HEADER_FLAG"
    }));
}

#[test]
fn preproc_macro_references_in_range_includes_undefined_conditionals() {
    let root_text = r#"`define KNOWN 1
`ifdef UNKNOWN
`endif
`ifndef KNOWN
`endif
module top;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let references =
        macro_references_in_range(&db, TOP, TextRange::up_to(TextSize::of(root_text))).unwrap();

    let unknown = references
        .iter()
        .find(|reference| reference.name.as_str() == "UNKNOWN")
        .expect("undefined conditional macro reference should be present");
    assert_eq!(text_at_range(root_text, unknown.range), "UNKNOWN");
    let unknown_definitions =
        macro_reference_definitions_at(&db, TOP, unknown.range.start()).unwrap().unwrap();
    assert!(unknown_definitions.definitions.is_empty());

    let known = references
        .iter()
        .find(|reference| reference.name.as_str() == "KNOWN")
        .expect("resolved conditional macro reference should be present");
    assert_eq!(text_at_range(root_text, known.range), "KNOWN");
    let known_definitions =
        macro_reference_definitions_at(&db, TOP, known.range.start()).unwrap().unwrap();
    assert!(!known_definitions.definitions.is_empty());
}

#[test]
fn preproc_project_header_guard_reference_is_indexed_without_include() {
    let root_text = "module top; endmodule\n";
    let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
    let db = db_with_files(root_text, header_text);
    let resolution =
        macro_reference_definitions_at(&db, HEADER, offset(header_text, "HEADER_FLAG"))
            .unwrap()
            .unwrap();

    assert!(resolution.references.iter().any(|reference| reference.file_id == HEADER));
    assert!(resolution.definitions.iter().any(|definition| {
        definition.file_id == HEADER
            && text_at_range(header_text, definition.name_range) == "HEADER_FLAG"
    }));
}
