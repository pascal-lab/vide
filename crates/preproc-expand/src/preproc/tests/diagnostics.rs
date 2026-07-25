use super::*;

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

    let crate::macro_file::Origin::MacroArg { arg_index, arg_range, .. } = target.origin else {
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

    let crate::macro_file::Origin::MacroBody { body_range, .. } = target.origin else {
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
