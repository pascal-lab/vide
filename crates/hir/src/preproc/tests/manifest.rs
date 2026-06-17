use super::*;

#[test]
fn preproc_manifest_predefine_definition_uses_manifest_provenance() {
    let root_text = r#"`ifdef Z_FROM_MANIFEST
module top;
localparam int W = `Z_FROM_MANIFEST;
endmodule
`endif
"#;
    let manifest_text = "defines = [\"A_OTHER=2\", \"Z_FROM_MANIFEST=1\"]\n";
    let manifest_range = TextRange::new(
        offset(manifest_text, "\"Z_FROM_MANIFEST=1\""),
        offset_after(manifest_text, "\"Z_FROM_MANIFEST=1\""),
    );
    let other_range = TextRange::new(
        offset(manifest_text, "\"A_OTHER=2\""),
        offset_after(manifest_text, "\"A_OTHER=2\""),
    );
    let predefine = Predefine::with_source(
        "Z_FROM_MANIFEST=1",
        PredefineSource { path: abs_path("vide.toml"), range: manifest_range },
    );
    let other_predefine = Predefine::with_source(
        "A_OTHER=2",
        PredefineSource { path: abs_path("vide.toml"), range: other_range },
    );
    let db = db_with_entries_and_predefine_entries(
        &[(TOP, "rtl/top.v", root_text), (MANIFEST, "vide.toml", manifest_text)],
        vec![other_predefine, predefine],
    );

    let resolution =
        macro_reference_definitions_at(&db, TOP, offset(root_text, "Z_FROM_MANIFEST;"))
            .unwrap()
            .unwrap();
    assert!(
        resolution.definitions.iter().any(|definition| {
            definition.file_id == MANIFEST && definition.name_range == manifest_range
        }),
        "predefine reference should target the manifest source range: {resolution:?}"
    );

    let definition = macro_definition_at(&db, MANIFEST, manifest_range.start()).unwrap().unwrap();
    assert_eq!(definition.file_id, MANIFEST);
    assert_eq!(definition.name.as_str(), "Z_FROM_MANIFEST");
    assert_eq!(definition.name_range, manifest_range);
    assert_eq!(text_at_range(manifest_text, definition.name_range), "\"Z_FROM_MANIFEST=1\"");

    let references = macro_references(&db, MANIFEST, &definition).unwrap();
    assert!(
        references.references.iter().any(|reference| {
            reference.file_id == TOP
                && text_at_range(root_text, reference.range) == "Z_FROM_MANIFEST"
        }),
        "manifest predefine definition should find source references: {references:?}"
    );
}

#[test]
fn preproc_manifest_escaped_predefine_definition_uses_manifest_provenance() {
    let root_text = r#"`ifdef MSG
module top;
localparam string S = `MSG;
endmodule
`endif
"#;
    let manifest_text = r#"defines = ["MSG=\"hello\""]
"#;
    let raw_define = r#""MSG=\"hello\"""#;
    let manifest_range =
        TextRange::new(offset(manifest_text, raw_define), offset_after(manifest_text, raw_define));
    let predefine = Predefine::with_source(
        r#"MSG="hello""#,
        PredefineSource { path: abs_path("vide.toml"), range: manifest_range },
    );
    let db = db_with_entries_and_predefine_entries(
        &[(TOP, "rtl/top.v", root_text), (MANIFEST, "vide.toml", manifest_text)],
        vec![predefine],
    );

    let definition = macro_definition_at(&db, MANIFEST, manifest_range.start()).unwrap().unwrap();
    assert_eq!(definition.file_id, MANIFEST);
    assert_eq!(definition.name.as_str(), "MSG");
    assert_eq!(definition.name_range, manifest_range);
    assert_eq!(text_at_range(manifest_text, definition.name_range), raw_define);

    let references = macro_references(&db, MANIFEST, &definition).unwrap();
    assert!(
        references.references.iter().any(|reference| reference.file_id == TOP
            && text_at_range(root_text, reference.range) == "MSG"),
        "escaped manifest predefine should still find source references: {references:?}"
    );
}

#[test]
fn preproc_visible_macro_names_follow_define_undef_boundaries() {
    let root_text = r#"`define A005_LOCAL 1
`undef A005_LOCAL
`define A005_NEXT 2
module top;
localparam int W = `A005_;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let names_after_define =
        visible_macro_names_at(&db, TOP, offset_after(root_text, "`define A005_LOCAL 1\n"))
            .unwrap();
    let names_after_undef =
        visible_macro_names_at(&db, TOP, offset_after(root_text, "`undef A005_LOCAL\n")).unwrap();
    let names_after_next =
        visible_macro_names_at(&db, TOP, offset_after(root_text, "`define A005_NEXT 2\n")).unwrap();

    assert!(names_after_define.iter().any(|name| name == "A005_LOCAL"));
    assert!(!names_after_undef.iter().any(|name| name == "A005_LOCAL"));
    assert!(names_after_next.iter().any(|name| name == "A005_NEXT"));
}

#[test]
fn preproc_inactive_branch_uses_header_define() {
    let root_text = r#"`include "defs.vh"
`ifndef HEADER_FLAG
wire disabled_by_header;
`endif
wire active;
"#;
    let header_text = "`define HEADER_FLAG\n";
    let db = db_with_files(root_text, header_text);

    let branches = inactive_branches(&db, TOP).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].file_id, TOP);
    assert!(text_at_range(root_text, branches[0].range).contains("disabled_by_header"));
}
