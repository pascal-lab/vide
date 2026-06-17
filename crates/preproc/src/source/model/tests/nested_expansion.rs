use super::*;

#[test]
fn source_model_builds_nested_expansion_graph_from_runtime_usage_records() {
    let root_text = r#"`define LEAF 3
`define WRAP `LEAF
module m;
localparam int W = `WRAP;
endmodule
"#;
    let (model, root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let wrap_reference = model
        .macro_references()
        .iter()
        .find(|reference| reference.name.as_str() == "WRAP")
        .expect("outer macro usage should create a reference");
    let wrap_call = model
        .macro_calls()
        .iter()
        .find(|call| call.reference == wrap_reference.id)
        .expect("outer macro usage should create a call");
    assert_eq!(wrap_call.call_range.source, root_source);

    let leaf_call = model
        .macro_calls()
        .iter()
        .find(|call| {
            let reference = model.macro_references().get(call.reference).unwrap();
            reference.name.as_str() == "LEAF"
                && matches!(reference.site, SourceMacroReferenceSite::Usage { .. })
        })
        .expect("nested macro invocation should create a runtime usage call");
    let leaf_reference = model.macro_references().get(leaf_call.reference).unwrap();
    assert_eq!(text_at_range(root_text, leaf_reference.name_range.range), "`LEAF");
    assert_eq!(leaf_call.parent_expansion_identity, wrap_call.expansion_identity);

    let SourceMacroExpansionQuery::Available(wrap_expansion_id) =
        model.immediate_macro_expansion(wrap_call.id)
    else {
        panic!("outer macro should have an expansion identity from the runtime usage record");
    };
    let wrap_expansion = model.macro_expansions().get(wrap_expansion_id).unwrap();
    assert_eq!(wrap_expansion.child_calls, vec![leaf_call.id]);

    let recursive = model.recursive_macro_expansion(wrap_call.id);
    assert_eq!(recursive.expansions.len(), 2);
    assert!(recursive.expansions.contains(&wrap_expansion_id));
    assert!(recursive.unavailable.is_empty());
}

#[test]
fn source_model_builds_nested_leaf_expansion_from_direct_identity() {
    let root_text = r#"`define LEAF 3
`define WRAP `LEAF
module m;
localparam int W = `WRAP;
endmodule
"#;
    let (model, _root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let leaf_call = model
        .macro_calls()
        .iter()
        .find(|call| {
            let reference = model.macro_references().get(call.reference).unwrap();
            reference.name.as_str() == "LEAF"
                && matches!(reference.site, SourceMacroReferenceSite::Usage { .. })
        })
        .expect("nested macro invocation should create a runtime usage call");
    assert!(leaf_call.identity.is_some());
    assert!(leaf_call.expansion_identity.is_some());
    assert!(leaf_call.parent_expansion_identity.is_some());

    let SourceMacroExpansionQuery::Available(leaf_expansion_id) =
        model.immediate_macro_expansion(leaf_call.id)
    else {
        panic!("nested macro should have its own immediate expansion");
    };
    let emitted = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "3")
        .expect("nested macro body token should be emitted");
    let SourceTokenProvenance::MacroBody { identity, definition, call, .. } =
        model.token_provenance().get(emitted.provenance.unwrap()).unwrap()
    else {
        panic!("nested emitted token should keep macro body provenance");
    };
    assert_eq!(*call, leaf_call.id);
    assert_eq!(Some(identity.call), leaf_call.identity);
    assert_eq!(Some(identity.expansion), leaf_call.expansion_identity);
    assert_eq!(identity.parent_expansion, leaf_call.parent_expansion_identity);
    assert_eq!(
        Some(identity.definition),
        model.macro_definitions().get(*definition).unwrap().identity
    );

    let recursive = model.recursive_macro_expansion(leaf_call.id);
    assert_eq!(recursive.expansions, vec![leaf_expansion_id]);
    assert!(recursive.unavailable.is_empty());
}

#[test]
fn source_model_keeps_macro_body_references_for_each_call_site() {
    let root_text = r#"`define LEAF 3
`define WRAP `LEAF
module m;
localparam int A = `WRAP;
localparam int B = `WRAP;
endmodule
"#;
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

#[test]
fn source_model_records_macro_operation_tokens_without_dropping_tokens() {
    let root_text = r#"`define JOIN(a,b) a``b
`define STR(x) `"x`"
module m;
wire `JOIN(foo,bar);
string s = `STR(foo);
endmodule
"#;
    let (model, _root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let pasted = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "foobar")
        .expect("token paste result should not be dropped");
    let SourceTokenProvenance::TokenPaste { call: paste_call, identity: paste_identity } =
        model.token_provenance().get(pasted.provenance.unwrap()).unwrap()
    else {
        panic!(
            "token paste should carry macro operation provenance: {:?}",
            model.token_provenance().get(pasted.provenance.unwrap()).unwrap()
        );
    };
    assert_eq!(
        Some(paste_identity.call),
        model.macro_calls().get(*paste_call).and_then(|call| call.identity)
    );

    let stringified = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "\"foo\"")
        .expect("stringification result should not be dropped");
    let SourceTokenProvenance::Stringification {
        call: stringification_call,
        identity: stringification_identity,
    } = model.token_provenance().get(stringified.provenance.unwrap()).unwrap()
    else {
        panic!("stringification should carry macro operation provenance");
    };
    assert_eq!(
        Some(stringification_identity.call),
        model.macro_calls().get(*stringification_call).and_then(|call| call.identity)
    );
    assert_ne!(paste_call, stringification_call);
    for call in [*paste_call, *stringification_call] {
        let SourceMacroExpansionQuery::Available(expansion) = model.immediate_macro_expansion(call)
        else {
            panic!("macro operation call should have an available expansion");
        };
        assert_ne!(model.macro_expansions().get(expansion).unwrap().emitted_token_range.len, 0);
    }
}

#[test]
fn source_model_links_pasted_macro_usage_to_parent_call() {
    let root_text = r#"`define FOOBAR 9
`define CALL(a,b) `a``b
module m;
localparam int W = `CALL(FOO,BAR);
endmodule
"#;
    let (model, _root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let parent_call = model
        .macro_calls()
        .iter()
        .find(|call| {
            model
                .macro_references()
                .get(call.reference)
                .is_some_and(|reference| reference.name.as_str() == "CALL")
        })
        .expect("CALL invocation should be recorded");
    let child_call = model
        .macro_calls()
        .iter()
        .find(|call| {
            model
                .macro_references()
                .get(call.reference)
                .is_some_and(|reference| reference.name.as_str() == "FOOBAR")
        })
        .expect("pasted macro usage should be expanded as a child call");
    assert_eq!(child_call.parent_expansion_identity, parent_call.expansion_identity);

    let SourceMacroExpansionQuery::Available(parent_expansion) =
        model.immediate_macro_expansion(parent_call.id)
    else {
        panic!("CALL invocation should have an immediate expansion");
    };
    let SourceMacroExpansionQuery::Available(child_expansion) =
        model.immediate_macro_expansion(child_call.id)
    else {
        panic!("pasted macro usage should have an immediate expansion");
    };

    let recursive = model.recursive_macro_expansion(parent_call.id);
    assert!(recursive.unavailable.is_empty());
    assert_eq!(recursive.expansions, vec![parent_expansion, child_expansion]);
    assert!(model.emitted_tokens().iter().any(|token| token.text.as_str() == "9"));
}
