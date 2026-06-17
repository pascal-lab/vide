use super::*;

#[test]
fn macro_generated_declaration_hir_range_resolves_to_diagnostic_provenance() {
    let root_text = r#"`define MAKE_DECL(name) logic name;
module top;
`MAKE_DECL(generated)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let (hir_file, _) = db.hir_file_with_source_map(TOP.into());
    let (local_module_id, _) = hir_file.modules.iter().next().unwrap();
    let module_id: ModuleId = InFile::new(TOP.into(), local_module_id);
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (declaration_id, _) =
        module.declarations.iter().next().expect("generated declaration should lower to HIR");
    let declaration_src = module_src_map
        .get(declaration_id)
        .expect("generated declaration should keep a source-map range");

    let provenance =
        diagnostic_provenance_for_range(&db, TOP, declaration_src.range()).unwrap().unwrap();

    assert!(matches!(provenance, DiagnosticProvenance::MacroBody { .. }));
}

#[test]
fn diagnostic_provenance_for_range_spanning_two_macro_calls_is_ambiguous() {
    let root_text = r#"`define A 1
`define B 2
module top;
localparam int W = `A + `B;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let range = TextRange::new(offset(root_text, "`A"), offset_after(root_text, "`B"));

    let provenance = diagnostic_provenance_for_range(&db, TOP, range).unwrap().unwrap();

    assert!(matches!(
        provenance,
        DiagnosticProvenance::Unavailable(PreprocUnavailable::AmbiguousDiagnosticProvenance {
            targets: 2
        })
    ));
}

#[test]
fn diagnostic_provenance_for_adjacent_macro_calls_only_hits_intersecting_call() {
    let root_text = r#"`define ID(x) x
module top;
localparam int W = `ID(1)`ID(2);
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let two_range = TextRange::new(offset(root_text, "`ID(2)"), offset_after(root_text, "`ID(2)"));

    let provenance = diagnostic_provenance_for_range(&db, TOP, two_range).unwrap().unwrap();

    let DiagnosticProvenance::MacroArgument { call, argument_index, file_id, range } = provenance
    else {
        panic!("adjacent single-call range should resolve precisely: {provenance:?}");
    };
    assert_eq!(text_at_range(root_text, call.range), "`ID(2)");
    assert_eq!(argument_index, 0);
    assert_eq!(file_id, TOP);
    assert_eq!(text_at_range(root_text, range), "2");
}

#[test]
fn diagnostic_provenance_for_nested_macro_call_range_is_precise() {
    let root_text = r#"`define LEAF 3
`define WRAP `LEAF
module top;
localparam int W = `WRAP;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let leaf_range = TextRange::new(offset(root_text, "`LEAF"), offset_after(root_text, "`LEAF"));

    let provenance = diagnostic_provenance_for_range(&db, TOP, leaf_range).unwrap().unwrap();

    let DiagnosticProvenance::MacroBody { call, file_id, range, .. } = provenance else {
        panic!("nested macro call range should resolve precisely");
    };
    assert_eq!(text_at_range(root_text, call.range), "`LEAF");
    assert_eq!(file_id, TOP);
    assert_eq!(text_at_range(root_text, range), "3");
}

#[test]
fn diagnostic_provenance_returns_unavailable_for_unsupported_expansion_mapping() {
    let root_text = r#"`define JOIN(a,b) a``b
`define STR(x) `"x`"
module top;
wire `JOIN(foo,bar);
string s = `STR(foo);
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let call_range =
        TextRange::new(offset(root_text, "`JOIN"), offset_after(root_text, "`JOIN(foo,bar)"));

    let provenance = diagnostic_provenance_for_range(&db, TOP, call_range).unwrap().unwrap();
    assert!(
        matches!(provenance, DiagnosticProvenance::Unavailable(_)),
        "token paste diagnostic provenance should be unavailable, got {provenance:?}"
    );

    let stringification_range =
        TextRange::new(offset(root_text, "`STR"), offset_after(root_text, "`STR(foo)"));
    let provenance =
        diagnostic_provenance_for_range(&db, TOP, stringification_range).unwrap().unwrap();
    assert!(
        matches!(provenance, DiagnosticProvenance::Unavailable(_)),
        "stringification diagnostic provenance should be unavailable, got {provenance:?}"
    );
}

#[test]
fn diagnostic_provenance_for_unbacked_predefine_expansion_is_structured_unavailable() {
    let root_text = r#"module top;
`MAKE_CHILD
endmodule
"#;
    let db = db_with_entries_and_predefines(
        &[(TOP, "rtl/top.v", root_text)],
        vec!["MAKE_CHILD=child u();".to_owned()],
    );
    let (hir_file, _) = db.hir_file_with_source_map(TOP.into());
    let (local_module_id, _) = hir_file.modules.iter().next().unwrap();
    let module_id: ModuleId = InFile::new(TOP.into(), local_module_id);
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (instantiation_id, _) = module
        .instantiations
        .iter()
        .next()
        .expect("predefine expansion should lower to a module instantiation");
    let instantiation_src = module_src_map
        .get(instantiation_id)
        .expect("generated instantiation should keep a source-map range");

    let provenance =
        diagnostic_provenance_for_range(&db, TOP, instantiation_src.range()).unwrap().unwrap();

    assert!(
        matches!(provenance, DiagnosticProvenance::Unavailable(_)),
        "unbacked predefine diagnostic provenance should be unavailable, got {provenance:?}"
    );
}
