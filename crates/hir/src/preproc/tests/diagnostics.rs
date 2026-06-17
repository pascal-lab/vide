use super::*;

#[test]
fn macro_generated_declaration_hir_range_resolves_to_diagnostic_target() {
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

    let target =
        diagnostic_target_for_range(&db, TOP, declaration_src.range()).unwrap().target.unwrap();

    assert!(matches!(target.origin, crate::hir_def::macro_file::Origin::MacroBody { .. }));
}

#[test]
fn diagnostic_target_for_range_spanning_two_macro_calls_fails_closed() {
    let root_text = r#"`define A 1
`define B 2
module top;
localparam int W = `A + `B;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let range = TextRange::new(offset(root_text, "`A"), offset_after(root_text, "`B"));

    let target = diagnostic_target_for_range(&db, TOP, range).unwrap();

    assert!(target.covered);
    assert!(target.target.is_none());
}

#[test]
fn diagnostic_target_for_adjacent_macro_calls_only_hits_intersecting_call() {
    let root_text = r#"`define ID(x) x
module top;
localparam int W = `ID(1)`ID(2);
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let two_range = TextRange::new(offset(root_text, "`ID(2)"), offset_after(root_text, "`ID(2)"));

    let target = diagnostic_target_for_range(&db, TOP, two_range).unwrap().target.unwrap();

    let crate::hir_def::macro_file::Origin::MacroArg { arg_index, arg_range, .. } = target.origin
    else {
        panic!("adjacent single-call range should resolve precisely: {target:?}");
    };
    assert_eq!(arg_index, 0);
    assert_eq!(target.file_id, TOP);
    assert_eq!(text_at_range(root_text, target.range), "2");
    assert_eq!(arg_range, target.range);
}

#[test]
fn diagnostic_target_for_nested_macro_call_range_is_precise() {
    let root_text = r#"`define LEAF 3
`define WRAP `LEAF
module top;
localparam int W = `WRAP;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let leaf_range = TextRange::new(offset(root_text, "`LEAF"), offset_after(root_text, "`LEAF"));

    let target = diagnostic_target_for_range(&db, TOP, leaf_range).unwrap().target.unwrap();

    let crate::hir_def::macro_file::Origin::MacroBody { body_range, .. } = target.origin else {
        panic!("nested macro call range should resolve precisely");
    };
    assert_eq!(target.file_id, TOP);
    assert_eq!(text_at_range(root_text, target.range), "3");
    assert_eq!(body_range, target.range);
}

#[test]
fn diagnostic_target_returns_none_for_unsupported_expansion_mapping() {
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

    let target = diagnostic_target_for_range(&db, TOP, call_range).unwrap();
    assert!(target.covered);
    assert!(target.target.is_none(), "token paste diagnostic target should fail closed");

    let stringification_range =
        TextRange::new(offset(root_text, "`STR"), offset_after(root_text, "`STR(foo)"));
    let target = diagnostic_target_for_range(&db, TOP, stringification_range).unwrap();
    assert!(target.covered);
    assert!(target.target.is_none(), "stringification diagnostic target should fail closed");
}

#[test]
fn diagnostic_target_for_unbacked_predefine_expansion_fails_closed() {
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

    let target = diagnostic_target_for_range(&db, TOP, instantiation_src.range()).unwrap();

    assert!(target.covered);
    assert!(target.target.is_none(), "unbacked predefine diagnostic target should fail closed");
}
