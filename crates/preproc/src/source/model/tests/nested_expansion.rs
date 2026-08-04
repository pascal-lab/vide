use super::*;

#[test]
fn source_model_keeps_macro_body_references_for_each_call_site() {
    let root_text = r#"""`define LEAF 3
`define WRAP `LEAF
module m;
localparam int A = `WRAP;
localparam int B = `WRAP;
endmodule
"""#;
    let (model, _root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let references = model
        .macro_references()
        .iter()
        .filter(|reference| {
            // Macro body references have no usage call pointing back at them;
            // every call site of WRAP records one reference per body token.
            reference.name.as_str() == "LEAF"
                && !model.macro_calls().iter().any(|call| call.reference == reference.id)
        })
        .collect::<Vec<_>>();

    assert_eq!(references.len(), 2);
    assert_eq!(references[0].name_range, references[1].name_range);
    assert_eq!(references[0].resolution, references[1].resolution);
}
