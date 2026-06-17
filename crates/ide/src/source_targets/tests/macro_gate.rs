use super::*;

#[test]
fn source_targets_macro_origin_gate_skips_plain_identifiers() {
    let text = "module m; wire payload_i; endmodule\n";

    assert!(!source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
}

#[test]
fn source_targets_macro_origin_gate_keeps_macro_names_and_arguments() {
    let text = "module m; wire `MAKE_DECL(payload_i); endmodule\n";

    assert!(source_macro_invocation_may_cover_offset(text, offset(text, "`MAKE_DECL")));
    assert!(source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
}

#[test]
fn source_targets_macro_origin_gate_keeps_outer_arguments_after_nested_macros() {
    let text = "assign y = `OUTER(a, `INNER(b), payload_i);\n";

    assert!(source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
}
