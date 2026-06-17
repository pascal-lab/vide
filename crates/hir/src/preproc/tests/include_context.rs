use super::*;

#[test]
fn preproc_nested_include_chain_maps_to_file_ids() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `LEAF_WIDTH;
endmodule
"#;
    let header_text = "`include \"leaf.vh\"\n";
    let leaf_text = "`define LEAF_WIDTH 4\n";
    let db = db_with_nested_files(root_text, header_text, leaf_text);

    let resolution =
        macro_usage_resolution_at(&db, TOP, offset(root_text, "LEAF_WIDTH")).unwrap().unwrap();

    assert_eq!(resolution.definition.file_id, LEAF);
    assert_eq!(resolution.definition_origin.file_id, LEAF);
    assert_eq!(resolution.include_chain.len(), 2);
    assert_eq!(resolution.include_chain[0].include_file_id, TOP);
    assert_eq!(resolution.include_chain[0].included_file_id, HEADER);
    assert!(
        text_at_range(root_text, resolution.include_chain[0].include_range).contains("defs.vh")
    );
    assert_eq!(resolution.include_chain[1].include_file_id, HEADER);
    assert_eq!(resolution.include_chain[1].included_file_id, LEAF);
    assert!(
        text_at_range(header_text, resolution.include_chain[1].include_range).contains("leaf.vh")
    );
}

#[test]
fn preproc_unsaved_include_buffer_updates_query_result() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
    let mut db = db_with_files(root_text, "`define OTHER_WIDTH 8\n");

    assert!(
        macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH")).unwrap().is_none()
    );

    db.set_file_text_with_durability(
        HEADER,
        Arc::from("`define HEADER_WIDTH 16\n"),
        Durability::LOW,
    );

    let resolution =
        macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH")).unwrap().unwrap();
    assert_eq!(resolution.definition.file_id, HEADER);
    assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
}

#[test]
fn preproc_visible_macro_names_include_predefines_without_file_mapping() {
    let root_text = r#"`define A005_LOCAL 1
module top;
localparam int W = `A005_;
endmodule
"#;
    let db = db_with_entries_and_predefines(
        &[(TOP, "rtl/top.v", root_text)],
        vec!["A005_MAGIC=42".to_owned()],
    );

    let names = visible_macro_names_at(&db, TOP, offset_after(root_text, "`A005_")).unwrap();

    assert!(names.iter().any(|name| name == "A005_LOCAL"), "{names:?}");
    assert!(names.iter().any(|name| name == "A005_MAGIC"), "{names:?}");
}

#[test]
fn preproc_single_offset_contexts_exclude_unrelated_profile_models() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let unrelated_header_text = "`define UNUSED_WIDTH 16\n";
    let db = db_with_nested_files(root_text, header_text, unrelated_header_text);

    let contexts = source_preproc_single_query_contexts(&db, HEADER);

    assert!(contexts.model_file_ids.contains(&TOP), "{contexts:?}");
    assert!(!contexts.model_file_ids.contains(&HEADER), "{contexts:?}");
    assert!(
        !contexts.model_file_ids.contains(&LEAF),
        "single-offset query contexts should not include unrelated profile model: {contexts:?}"
    );
}

#[test]
fn preproc_include_only_sv_query_uses_all_including_roots() {
    let top_a_text = r#"`define WIDTH 8
`include "shared.sv"
"#;
    let shared_text = "localparam int W = `WIDTH;\n";
    let top_b_text = r#"`define WIDTH 16
`include "shared.sv"
"#;
    let db = db_with_entries(&[
        (TOP, "rtl/top_a.sv", top_a_text),
        (HEADER, "include/shared.sv", shared_text),
        (LEAF, "rtl/top_b.sv", top_b_text),
    ]);

    let plan = db.compilation_plan_for_profile(Some(PROFILE));
    assert!(plan.include_only.contains(&HEADER), "{plan:?}");
    assert!(plan.roots.contains(&TOP), "{plan:?}");
    assert!(plan.roots.contains(&LEAF), "{plan:?}");
    assert!(!plan.roots.contains(&HEADER), "{plan:?}");

    let contexts = source_preproc_single_query_contexts(&db, HEADER);
    assert!(contexts.model_file_ids.contains(&TOP), "{contexts:?}");
    assert!(contexts.model_file_ids.contains(&LEAF), "{contexts:?}");
    assert!(!contexts.model_file_ids.contains(&HEADER), "{contexts:?}");

    let definitions =
        macro_reference_definitions_at(&db, HEADER, offset(shared_text, "WIDTH")).unwrap().unwrap();

    assert_eq!(definitions.definitions.len(), 2);
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP && text_at_range(top_a_text, definition.name_range) == "WIDTH"
    }));
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == LEAF && text_at_range(top_b_text, definition.name_range) == "WIDTH"
    }));
}

#[test]
fn preproc_header_query_uses_including_context_over_standalone_model() {
    let root_text = r#"`define FEATURE 1
`include "defs.vh"
"#;
    let header_text = r#"`ifdef FEATURE
`define WIDTH 8
`endif
localparam int W = `WIDTH;
"#;
    let db = db_with_files(root_text, header_text);

    let reference = macro_reference_at(&db, HEADER, offset(header_text, "WIDTH;"))
        .unwrap()
        .expect("included context should resolve the header reference without ambiguity");
    assert_eq!(text_at_range(header_text, reference.range), "`WIDTH");
    assert!(matches!(reference.resolution, MacroResolution::Resolved { .. }));

    let resolution = macro_reference_resolution_at(&db, HEADER, offset(header_text, "WIDTH;"))
        .unwrap()
        .expect("header macro reference should resolve through the including root");
    assert_eq!(resolution.definition.file_id, HEADER);
    assert_eq!(text_at_range(header_text, resolution.definition.name_range), "WIDTH");
}

#[test]
fn preproc_header_without_including_context_uses_standalone_model() {
    let root_text = "module top; endmodule\n";
    let header_text = "`define WIDTH 8\n";
    let db = db_with_files(root_text, header_text);

    let contexts = source_preproc_single_query_contexts(&db, HEADER);

    assert!(contexts.model_file_ids.contains(&HEADER), "{contexts:?}");
    assert!(!contexts.model_file_ids.contains(&TOP), "{contexts:?}");
}

#[test]
fn preproc_partial_context_index_is_structured_unavailable() {
    let contexts = SourcePreprocQueryContexts {
        model_file_ids: Vec::new(),
        status: SourcePreprocContextStatus::Partial { skipped_models: 2 },
    };

    let error = finish_empty_single_query(&contexts, None).unwrap_err();

    assert!(matches!(error, PreprocError::PartialPreprocContextIndex { skipped_models: 2 }));
}
