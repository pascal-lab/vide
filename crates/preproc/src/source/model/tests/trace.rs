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
    assert_eq!(wrap_call.parent_trace_expansion, next_call.trace_expansion);
    assert_eq!(leaf_call.parent_trace_expansion, wrap_call.trace_expansion);

    let Ok(next_expansion_id) = model.immediate_macro_expansion(next_call.id) else {
        panic!("NEXT should have an immediate expansion");
    };
    let Ok(wrap_expansion_id) = model.immediate_macro_expansion(wrap_call.id) else {
        panic!("WRAP should have an immediate expansion");
    };
    let Ok(leaf_expansion_id) = model.immediate_macro_expansion(leaf_call.id) else {
        panic!("LEAF should have an immediate expansion");
    };

    let next_expansion = model.macro_expansions().get(next_expansion_id).unwrap();
    assert_eq!(next_expansion.child_calls, vec![wrap_call.id]);
    let wrap_expansion = model.macro_expansions().get(wrap_expansion_id).unwrap();
    assert_eq!(wrap_expansion.child_calls, vec![leaf_call.id]);
    let leaf_expansion = model.macro_expansions().get(leaf_expansion_id).unwrap();
    assert!(leaf_expansion.child_calls.is_empty());

    let (payload, body_token_range) = model
        .emitted_tokens()
        .iter()
        .find_map(|token| {
            let SourceTokenOrigin::MacroBody { call, body_token_range, .. } =
                model.token_origins().get(token.origin?)?
            else {
                return None;
            };
            (*call == leaf_call.id).then_some((token, *body_token_range))
        })
        .expect("final payload token should keep LEAF body origin");
    assert_eq!(payload.text.as_str(), "payload_i");
    assert_eq!(leaf_call.parent_trace_expansion, wrap_call.trace_expansion);
    assert_eq!(text_at_range(root_text, body_token_range.range), "payload_i");
}

#[test]
fn source_model_preserves_multi_token_argument_direct_trace() {
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
            let SourceTokenOrigin::MacroArgument {
                trace_call,
                call,
                argument_index,
                body_token_range,
                argument_token_range,
                argument_token_index,
                ..
            } = model.token_origins().get(token.origin?)?
            else {
                return None;
            };
            (token.text.as_str() == "payload_i").then_some((
                *trace_call,
                *call,
                *argument_index,
                *body_token_range,
                *argument_token_range,
                *argument_token_index,
            ))
        })
        .expect("payload identifier should be direct macro argument origin");
    let slice = model
        .emitted_tokens()
        .iter()
        .find_map(|token| {
            let SourceTokenOrigin::MacroArgument {
                trace_call,
                call,
                argument_index,
                body_token_range,
                argument_token_range,
                argument_token_index,
                ..
            } = model.token_origins().get(token.origin?)?
            else {
                return None;
            };
            (token.text.as_str() == "3").then_some((
                *trace_call,
                *call,
                *argument_index,
                *body_token_range,
                *argument_token_range,
                *argument_token_index,
            ))
        })
        .expect("slice index should be direct macro argument origin");

    assert_eq!(payload.0, slice.0);
    assert_eq!(payload.1, slice.1);
    assert_eq!(payload.2, 0);
    assert_eq!(slice.2, 0);
    assert_eq!(payload.5, 0);
    assert_eq!(slice.5, 2);
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
