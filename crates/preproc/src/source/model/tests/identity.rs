use super::*;

#[test]
fn source_model_preserves_nested_actual_argument_macro_parent_chain() {
    let root_text = r#"`define LEAF payload_i
`define WRAP `LEAF
`define NEXT(x) ((x) + 12'd1)
module m(input logic [3:0] payload_i, output logic [3:0] y);
assign y = `NEXT(`WRAP);
endmodule
"#;
    let (model, _root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let call_by_name = |name: &str| {
        model
            .macro_calls()
            .iter()
            .find(|call| {
                let reference = model.macro_references().get(call.reference).unwrap();
                reference.name.as_str() == name
                    && matches!(reference.site, SourceMacroReferenceSite::Usage { .. })
            })
            .unwrap_or_else(|| panic!("{name} usage should create a call"))
    };

    let next_call = call_by_name("NEXT");
    let wrap_call = call_by_name("WRAP");
    let leaf_call = call_by_name("LEAF");
    assert_eq!(wrap_call.parent_expansion_identity, next_call.expansion_identity);
    assert_eq!(leaf_call.parent_expansion_identity, wrap_call.expansion_identity);

    let SourceMacroExpansionQuery::Available(next_expansion_id) =
        model.immediate_macro_expansion(next_call.id)
    else {
        panic!("NEXT should have an immediate expansion");
    };
    let SourceMacroExpansionQuery::Available(wrap_expansion_id) =
        model.immediate_macro_expansion(wrap_call.id)
    else {
        panic!("WRAP should have an immediate expansion");
    };
    let SourceMacroExpansionQuery::Available(leaf_expansion_id) =
        model.immediate_macro_expansion(leaf_call.id)
    else {
        panic!("LEAF should have an immediate expansion");
    };

    let next_recursive = model.recursive_macro_expansion(next_call.id);
    assert!(next_recursive.expansions.contains(&next_expansion_id));
    assert!(next_recursive.expansions.contains(&wrap_expansion_id));
    assert!(next_recursive.expansions.contains(&leaf_expansion_id));
    assert!(next_recursive.unavailable.is_empty());

    let (payload, identity, body_token_range) = model
        .emitted_tokens()
        .iter()
        .find_map(|token| {
            let SourceTokenProvenance::MacroBody { call, identity, body_token_range, .. } =
                model.token_provenance().get(token.provenance?)?
            else {
                return None;
            };
            (*call == leaf_call.id).then_some((token, *identity, *body_token_range))
        })
        .expect("final payload token should keep LEAF body provenance");
    assert_eq!(payload.text.as_str(), "payload_i");
    assert_eq!(identity.parent_expansion, wrap_call.expansion_identity);
    assert_eq!(text_at_range(root_text, body_token_range.range), "payload_i");
}

#[test]
fn source_model_preserves_multi_token_argument_direct_identity() {
    let root_text = r#"`define NEXT(x) ((x) + 12'd1)
module m(input logic [3:0] payload_i, output logic [3:0] y);
assign y = `NEXT(payload_i[3:0]);
endmodule
"#;
    let (model, root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let payload = model
        .emitted_tokens()
        .iter()
        .find_map(|token| {
            let SourceTokenProvenance::MacroArgument {
                identity,
                call,
                argument_index,
                body_token_range,
                argument_token_range,
            } = model.token_provenance().get(token.provenance?)?
            else {
                return None;
            };
            (token.text.as_str() == "payload_i").then_some((
                *identity,
                *call,
                *argument_index,
                *body_token_range,
                *argument_token_range,
            ))
        })
        .expect("payload identifier should be direct macro argument provenance");
    let slice = model
        .emitted_tokens()
        .iter()
        .find_map(|token| {
            let SourceTokenProvenance::MacroArgument {
                identity,
                call,
                argument_index,
                body_token_range,
                argument_token_range,
            } = model.token_provenance().get(token.provenance?)?
            else {
                return None;
            };
            (token.text.as_str() == "3").then_some((
                *identity,
                *call,
                *argument_index,
                *body_token_range,
                *argument_token_range,
            ))
        })
        .expect("slice index should be direct macro argument provenance");

    assert_eq!(payload.0.call, slice.0.call);
    assert_eq!(payload.1, slice.1);
    assert_eq!(payload.2, 0);
    assert_eq!(slice.2, 0);
    assert_eq!(payload.0.argument_token_index, 0);
    assert_eq!(slice.0.argument_token_index, 2);
    assert_eq!(payload.3, slice.3);
    assert_eq!(payload.4.source, root_source);
    assert_eq!(slice.4.source, root_source);
    let call = model.macro_calls().get(payload.1).unwrap();
    assert_eq!(call.arguments.len(), 1);
    assert_eq!(
        text_at_range(root_text, call.arguments[0].argument_range.unwrap().range),
        "payload_i[3:0]"
    );
}
