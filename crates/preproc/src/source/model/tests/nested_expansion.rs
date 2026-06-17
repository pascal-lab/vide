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
    assert_eq!(leaf_call.parent_trace_expansion, wrap_call.trace_expansion);

    let Ok(wrap_expansion_id) = model.immediate_macro_expansion(wrap_call.id) else {
        panic!("outer macro should have an expansion trace id from the runtime usage record");
    };
    let wrap_expansion = model.macro_expansions().get(wrap_expansion_id).unwrap();
    assert_eq!(wrap_expansion.child_calls, vec![leaf_call.id]);

    let Ok(leaf_expansion_id) = model.immediate_macro_expansion(leaf_call.id) else {
        panic!("nested macro should have its own immediate expansion");
    };
    let leaf_expansion = model.macro_expansions().get(leaf_expansion_id).unwrap();
    assert!(leaf_expansion.child_calls.is_empty());
}

#[test]
fn source_model_builds_nested_leaf_expansion_from_direct_trace() {
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
    assert!(leaf_call.trace_call.is_some());
    assert!(leaf_call.trace_expansion.is_some());
    assert!(leaf_call.parent_trace_expansion.is_some());

    let Ok(leaf_expansion_id) = model.immediate_macro_expansion(leaf_call.id) else {
        panic!("nested macro should have its own immediate expansion");
    };
    let emitted = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "3")
        .expect("nested macro body token should be emitted");
    let SourceTokenOrigin::MacroBody { origin, definition, call, .. } =
        model.token_origins().get(emitted.origin.unwrap()).unwrap()
    else {
        panic!("nested emitted token should keep macro body origin");
    };
    assert_eq!(*call, leaf_call.id);
    assert_eq!(Some(origin.call_id), leaf_call.trace_call);
    assert_eq!(Some(origin.expansion_id), leaf_call.trace_expansion);
    assert_eq!(origin.parent_expansion_id, leaf_call.parent_trace_expansion);
    assert_eq!(
        Some(origin.definition_id),
        model.macro_definitions().get(*definition).unwrap().trace_definition
    );

    let leaf_expansion = model.macro_expansions().get(leaf_expansion_id).unwrap();
    assert_eq!(leaf_expansion.call, leaf_call.id);
    assert!(leaf_expansion.child_calls.is_empty());
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
    let SourceTokenOrigin::TokenPaste { call: paste_call, origin: paste_origin } =
        model.token_origins().get(pasted.origin.unwrap()).unwrap()
    else {
        panic!(
            "token paste should carry macro operation origin: {:?}",
            model.token_origins().get(pasted.origin.unwrap()).unwrap()
        );
    };
    assert_eq!(
        Some(paste_origin.call_id),
        model.macro_calls().get(*paste_call).and_then(|call| call.trace_call)
    );

    let stringified = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "\"foo\"")
        .expect("stringification result should not be dropped");
    let SourceTokenOrigin::Stringify { call: stringification_call, origin: stringification_origin } =
        model.token_origins().get(stringified.origin.unwrap()).unwrap()
    else {
        panic!("stringification should carry macro operation origin");
    };
    assert_eq!(
        Some(stringification_origin.call_id),
        model.macro_calls().get(*stringification_call).and_then(|call| call.trace_call)
    );
    assert_ne!(paste_call, stringification_call);
    for call in [*paste_call, *stringification_call] {
        let Ok(expansion) = model.immediate_macro_expansion(call) else {
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
    assert_eq!(child_call.parent_trace_expansion, parent_call.trace_expansion);

    let Ok(parent_expansion) = model.immediate_macro_expansion(parent_call.id) else {
        panic!("CALL invocation should have an immediate expansion");
    };
    let Ok(child_expansion) = model.immediate_macro_expansion(child_call.id) else {
        panic!("pasted macro usage should have an immediate expansion");
    };

    let parent_expansion = model.macro_expansions().get(parent_expansion).unwrap();
    assert_eq!(parent_expansion.child_calls, vec![child_call.id]);
    let child_expansion = model.macro_expansions().get(child_expansion).unwrap();
    assert_eq!(child_expansion.call, child_call.id);
    assert!(child_expansion.child_calls.is_empty());
    assert!(model.emitted_tokens().iter().any(|token| token.text.as_str() == "9"));
}
