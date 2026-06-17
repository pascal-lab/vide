use super::*;

#[test]
fn preproc_macro_expansion_exposes_display_virtual_source() {
    let root_text = r#"`define MAKE_DECL(name) logic name;
module top;
`MAKE_DECL(generated)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let immediate =
        immediate_macro_expansion_at(&db, TOP, offset(root_text, "`MAKE_DECL")).unwrap().unwrap();
    let MacroExpansionQuery::Available(expansion) = immediate else {
        panic!("MAKE_DECL expansion should be available");
    };
    let MappedPreprocSource::VirtualDisplay { path, origin } = &expansion.display_source else {
        panic!("macro expansion should expose a display-only virtual expansion source");
    };
    assert_eq!(
        path,
        &VfsPath::new_virtual_path("/__vide/preproc/profile-0/expansion/0.sv".to_owned())
    );
    assert_eq!(
        origin,
        &PreprocVirtualOrigin::Expansion { expansion: SourceMacroExpansionId::new(0) }
    );

    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().unwrap();
    let expansion_display =
        mapped.source_map.expansion_display_text(SourceMacroExpansionId::new(0)).unwrap();
    assert_eq!(expansion_display, "\nlogic generated;");
    assert_eq!(expansion.display_text, expansion_display);
    assert_eq!(expansion.display_range, TextRange::new(1.into(), 17.into()));
}

#[test]
fn preproc_macro_expansion_display_keeps_emitted_token_trivia() {
    let root_text = r#"`define BLOCK(name) \
  always_ff @(posedge clk) begin \
    name <= 1; \
  end
module top;
  `BLOCK(q)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let immediate =
        immediate_macro_expansion_at(&db, TOP, offset(root_text, "`BLOCK")).unwrap().unwrap();
    let MacroExpansionQuery::Available(expansion) = immediate else {
        panic!("BLOCK expansion should be available");
    };
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().unwrap();
    let display_text = mapped
        .source_map
        .expansion_display_text(SourceMacroExpansionId::new(expansion.id.raw()))
        .unwrap();

    assert_eq!(expansion.display_text, display_text);
    assert!(
        display_text.contains("\n  always_ff")
            && display_text.contains("\n    q <= 1;")
            && display_text.contains("\n  end"),
        "expansion display text should preserve emitted token trivia: {display_text:?}"
    );
}

#[test]
fn preproc_maps_nested_actual_argument_macro_usage_without_dropping_expansion() {
    let root_text = r#"`define PAYL payload_i
`define NEXT(x) ((x) + 12'd1)
module top(input logic [3:0] payload_i, output logic [3:0] y);
assign y = `NEXT(`PAYL);
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let payl = macro_reference_definitions_at(&db, TOP, offset_after(root_text, "`NEXT("))
        .unwrap()
        .expect("nested actual-argument macro reference should be mapped");
    assert_eq!(text_at_range(root_text, payl.range), "`PAYL");
    assert!(
        payl.definitions
            .iter()
            .any(|definition| { definition.file_id == TOP && definition.name.as_str() == "PAYL" })
    );

    let next = immediate_macro_expansion_at(&db, TOP, offset(root_text, "`NEXT")).unwrap().unwrap();
    let MacroExpansionQuery::Available(next) = next else {
        panic!("NEXT expansion should be available");
    };
    let argument = next
        .call
        .arguments
        .iter()
        .find(|argument| argument.argument_index == 0)
        .expect("NEXT call should expose its written actual argument");
    assert_eq!(text_at_range(root_text, argument.range.unwrap()), "`PAYL");
    assert_eq!(
        argument.tokens.iter().map(|token| token.raw.as_str()).collect::<Vec<_>>(),
        vec!["`PAYL"]
    );

    let payl_offset = offset(root_text, "`PAYL");
    let queries = macro_expansion_queries_at(&db, TOP, payl_offset).unwrap();
    assert!(queries.iter().any(|query| matches!(
        query,
        MacroExpansionQuery::Available(expansion)
            if expansion.definition.name().as_str() == "NEXT"
    )));
    assert!(queries.iter().any(|query| matches!(
        query,
        MacroExpansionQuery::Available(expansion)
            if expansion.definition.name().as_str() == "PAYL"
    )));
    assert!(!queries.iter().any(|query| matches!(query, MacroExpansionQuery::Unavailable(_))));
    assert!(matches!(
        immediate_macro_expansion_at(&db, TOP, payl_offset),
        Ok(Some(MacroExpansionQuery::Ambiguous(expansions)))
            if expansions.len() == 2
                && expansions.iter().any(|expansion| expansion.definition.name().as_str() == "NEXT")
                && expansions.iter().any(|expansion| expansion.definition.name().as_str() == "PAYL")
    ));
}

#[test]
fn preproc_numeric_literal_expansion_display_is_not_source_buffer() {
    let root_text = r#"`define ONE 12'd1
module top;
localparam int W = `ONE;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let immediate =
        immediate_macro_expansion_at(&db, TOP, offset(root_text, "`ONE")).unwrap().unwrap();
    let MacroExpansionQuery::Available(expansion) = immediate else {
        panic!("ONE expansion should be available");
    };
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().unwrap();
    assert_expansion_is_display_only_source_buffer(mapped, &expansion);

    let display_text = mapped
        .source_map
        .expansion_display_text(SourceMacroExpansionId::new(expansion.id.raw()))
        .unwrap();
    assert!(display_text.contains("12"));
    assert!(display_text.contains("'d"));
    assert!(display_text.contains("1"));
}

#[test]
fn preproc_escaped_identifier_expansion_display_is_not_source_buffer() {
    let root_text = concat!(
        "`define ESCAPED \\escaped.name \n",
        "module top;\n",
        "wire `ESCAPED;\n",
        "endmodule\n",
    );
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let immediate =
        immediate_macro_expansion_at(&db, TOP, offset(root_text, "`ESCAPED")).unwrap().unwrap();
    let MacroExpansionQuery::Available(expansion) = immediate else {
        panic!("ESCAPED expansion should be available");
    };
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().unwrap();
    assert_expansion_is_display_only_source_buffer(mapped, &expansion);

    let display_text = mapped
        .source_map
        .expansion_display_text(SourceMacroExpansionId::new(expansion.id.raw()))
        .unwrap();
    assert!(display_text.contains("\\escaped.name"));
}
