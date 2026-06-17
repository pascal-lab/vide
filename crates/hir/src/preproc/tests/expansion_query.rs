use super::*;

#[test]
fn preproc_include_usage_resolves_to_header_define() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let db = db_with_files(root_text, header_text);

    let resolution =
        macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH")).unwrap().unwrap();
    assert_eq!(resolution.usage.file_id, TOP);
    assert_eq!(resolution.definition.file_id, HEADER);
    assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
    assert_eq!(text_at_range(header_text, resolution.definition.name_range), "HEADER_WIDTH");

    let include = include_directive_at(&db, TOP, offset(root_text, "defs.vh")).unwrap().unwrap();
    assert_eq!(text_at_range(root_text, include.range), "\"defs.vh\"");
    assert!(include_directive_at(&db, TOP, offset(root_text, "`include")).unwrap().is_none());
    assert!(include_directive_at(&db, TOP, include.range.end()).unwrap().is_none());
    let IncludeTarget::Literal { resolved_file, .. } = include.target else {
        panic!("literal include expected");
    };
    assert_eq!(resolved_file, Some(HEADER));
}

#[test]
fn preproc_macro_expansion_queries_map_call_ranges() {
    let root_text = r#"`define OBJ 8
`define LEAF 3
`define WRAP `LEAF
module top;
localparam int A = `OBJ;
localparam int B = `WRAP;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let immediate =
        immediate_macro_expansion_at(&db, TOP, offset(root_text, "`OBJ")).unwrap().unwrap();
    let MacroExpansionQuery::Available(immediate) = immediate else {
        panic!("object-like macro expansion should be available");
    };
    assert_eq!(immediate.call.file_id, TOP);
    assert_eq!(text_at_range(root_text, immediate.call.range), "`OBJ");
    assert_eq!(immediate.emitted_token_range.len, 1);

    let recursive =
        recursive_macro_expansion_at(&db, TOP, offset(root_text, "`WRAP")).unwrap().unwrap();
    assert_eq!(recursive.root_call.file_id, TOP);
    assert_eq!(text_at_range(root_text, recursive.root_call.range), "`WRAP");
    assert!(recursive.unavailable.is_empty());
    assert_eq!(recursive.expansions.len(), 2);
    let wrap_expansion = recursive
        .expansions
        .iter()
        .find(|expansion| expansion.definition.name().as_str() == "WRAP")
        .expect("outer expansion should be mapped");
    let leaf_expansion = recursive
        .expansions
        .iter()
        .find(|expansion| expansion.definition.name().as_str() == "LEAF")
        .expect("nested expansion should be mapped");
    assert_eq!(text_at_range(root_text, wrap_expansion.call.range), "`WRAP");
    assert_eq!(text_at_range(root_text, leaf_expansion.call.range), "`LEAF");
    assert_eq!(wrap_expansion.child_calls, vec![leaf_expansion.call.id]);
}

#[test]
fn preproc_macro_call_resolutions_in_range_map_formal_params() {
    let root_text = "\
`define MAKE(width, expr) logic [width-1:0] expr
module top; `MAKE(8, data_q) endmodule
";
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let start = offset(root_text, "`MAKE");
    let end = offset_after(root_text, "data_q");

    let resolutions =
        macro_call_resolutions_in_range(&db, TOP, TextRange::new(start, end)).unwrap();

    assert_eq!(resolutions.len(), 1);
    let resolution = &resolutions[0];
    assert_eq!(text_at_range(root_text, resolution.call.range), "`MAKE(8, data_q)");
    assert_eq!(
        resolution
            .definition
            .params
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|param| param.name.as_deref())
            .collect::<Vec<_>>(),
        vec!["width", "expr"]
    );
    assert_eq!(
        resolution
            .call
            .arguments
            .iter()
            .filter_map(|argument| argument.range.map(|range| text_at_range(root_text, range)))
            .collect::<Vec<_>>(),
        vec!["8", "data_q"]
    );
}

#[test]
fn preproc_builtin_intrinsic_expansion_uses_structured_diagnostic_provenance() {
    let root_text = r#"module m;
localparam int L = `__LINE__;
localparam string F = `__FILE__;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let line_offset = offset(root_text, "`__LINE__");
    let file_offset = offset(root_text, "`__FILE__");
    for (offset, expected_name) in [(line_offset, "__LINE__"), (file_offset, "__FILE__")] {
        let immediate =
            immediate_macro_expansion_at(&db, TOP, offset).unwrap().expect("builtin call expected");
        let MacroExpansionQuery::Available(immediate) = immediate else {
            panic!("builtin macro expansion should be available");
        };
        assert_eq!(immediate.definition.name().as_str(), expected_name);
        assert!(matches!(
            immediate.definition,
            MacroExpansionDefinition::Builtin { name, .. } if name.as_str() == expected_name
        ));

        let recursive =
            recursive_macro_expansion_at(&db, TOP, offset).unwrap().expect("recursive expected");
        assert!(recursive.unavailable.is_empty());
        assert!(recursive.expansions.iter().any(|expansion| {
            matches!(
                &expansion.definition,
                MacroExpansionDefinition::Builtin { name, .. } if name.as_str() == expected_name
            )
        }));

        let diagnostic = diagnostic_provenance_for_range(&db, TOP, immediate.call.range)
            .unwrap()
            .expect("diagnostic provenance expected");
        assert!(matches!(
            diagnostic,
            DiagnosticProvenance::Builtin { name, call }
                if name.as_str() == expected_name && call.range == immediate.call.range
        ));
    }
}

#[test]
fn preproc_zero_token_macro_expansion_is_available() {
    let root_text = r#"`define EMPTY
`define DROP(x)
module top;
`EMPTY
`DROP(foo)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    for name in ["`EMPTY", "`DROP"] {
        let immediate =
            immediate_macro_expansion_at(&db, TOP, offset(root_text, name)).unwrap().unwrap();
        let MacroExpansionQuery::Available(immediate) = immediate else {
            panic!("{name} expansion should be available");
        };
        assert_eq!(immediate.emitted_token_range.len, 0);

        let mapped = db.source_preproc_model(TOP);
        let mapped = mapped.as_ref().as_ref().unwrap();
        let display_text = mapped
            .source_map
            .expansion_display_text(SourceMacroExpansionId::new(immediate.id.raw()))
            .unwrap();
        assert_eq!(display_text, "");
        assert_eq!(immediate.display_range, TextRange::empty(TextSize::from(0)));
    }
}
