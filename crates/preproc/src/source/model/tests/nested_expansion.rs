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
            reference.name.as_str() == "LEAF"
                && matches!(reference.site, SourceMacroReferenceSite::MacroBodyToken { .. })
        })
        .collect::<Vec<_>>();

    assert_eq!(references.len(), 2);
    let first_site = references[0].site;
    let second_site = references[1].site;
    let (
        SourceMacroReferenceSite::MacroBodyToken { call: first_call, token_index: first_token },
        SourceMacroReferenceSite::MacroBodyToken { call: second_call, token_index: second_token },
    ) = (first_site, second_site)
    else {
        unreachable!();
    };
    assert_ne!(first_call, second_call);
    assert_eq!(first_token, second_token);
    assert_eq!(references[0].name_range, references[1].name_range);
    assert_eq!(references[0].resolution, references[1].resolution);
}
