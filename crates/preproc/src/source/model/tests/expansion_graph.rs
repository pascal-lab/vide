use super::*;

#[test]
fn source_model_exposes_expansion_origin_tables() {
    let root_text = r#"`include "defs.vh"
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let definition = model
        .macro_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "HEADER_WIDTH")
        .expect("definition table should include precise macro definition");
    assert_eq!(definition.directive_range.source, header_source);
    assert_eq!(definition.name_range.source, header_source);
    assert_ne!(definition.directive_range.range, definition.name_range.range);
    assert_eq!(text_at_range(header_text, definition.name_range.range), "HEADER_WIDTH");

    let reference = model
        .macro_references()
        .iter()
        .find(|reference| {
            reference.name.as_str() == "HEADER_WIDTH"
                && matches!(reference.site, SourceMacroReferenceSite::Usage { usage_index: _ })
        })
        .expect("reference table should include resolved macro usage");
    assert_eq!(reference.name_range.source, root_source);
    assert_eq!(reference.directive_range.source, root_source);
    let SourceMacroResolution::Resolved { definition: resolved_definition, reason, include_chain } =
        &reference.resolution
    else {
        panic!("macro usage should resolve to included definition");
    };
    assert_eq!(*reason, SourceMacroResolutionReason::VisibleDefinition);
    assert_eq!(include_chain.len(), 1);
    assert_eq!(
        model.macro_definitions().get(*resolved_definition).unwrap().name.as_str(),
        "HEADER_WIDTH"
    );

    assert_eq!(model.include_graph().directives().len(), 1);
    assert!(matches!(
        &model.include_graph().directives()[0].status,
        SourceIncludeStatus::Resolved { source } if *source == header_source
    ));
    assert!(!model.state_timeline().checkpoints().is_empty());

    let call = model
        .macro_calls()
        .iter()
        .find(|call| call.reference == reference.id)
        .expect("macro usage should create a call record");
    assert_eq!(call.call_range.source, root_source);
    let Ok(expansion_id) = model.immediate_macro_expansion(call.id) else {
        panic!("object-like macro call should have an immediate expansion");
    };
    assert_eq!(call.expansion, Ok(expansion_id));
    let expansion = model.macro_expansions().get(expansion_id).unwrap();
    assert_eq!(expansion.call, call.id);
    assert_eq!(expansion.definition, SourceMacroExpansionDefinition::Source(*resolved_definition));
    assert!(expansion.child_calls.is_empty());

    let emitted = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "8")
        .expect("macro body token should be present in trace-emitted tokens");
    assert_eq!(expansion.emitted_token_range.start, emitted.id);
    assert_eq!(expansion.emitted_token_range.len, 1);
    let origin = model.token_origins().get(emitted.origin.unwrap()).unwrap();
    assert!(matches!(
        origin,
        SourceTokenOrigin::MacroBody {
            definition: body_definition,
            body_token_range,
            call: body_call,
            ..
        } if *body_definition == *resolved_definition
            && body_token_range.source == header_source
            && *body_call == call.id
    ));
}

#[test]
fn source_model_maps_function_macro_argument_emitted_token_to_argument() {
    let root_text = r#"`define ID(x) x
module m;
localparam int W = `ID(7);
endmodule
"#;
    let (model, root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let emitted = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "7")
        .expect("argument replacement token should be emitted");
    let SourceTokenOrigin::MacroArgument { call, argument_index, argument_token_range, .. } =
        model.token_origins().get(emitted.origin.unwrap()).unwrap()
    else {
        panic!("argument replacement should map to MacroArgument origin");
    };
    assert_eq!(*argument_index, 0);
    assert_eq!(argument_token_range.source, root_source);
    assert_eq!(text_at_range(root_text, argument_token_range.range), "7");

    let call = model.macro_calls().get(*call).expect("call id should resolve");
    assert_eq!(call.call_range.source, root_source);
    assert_eq!(text_at_range(root_text, call.call_range.range), "`ID(7)");
    assert_eq!(call.arguments.len(), 1);
    assert_eq!(call.arguments[0].argument_index, 0);
    assert_eq!(call.arguments[0].argument_range, Some(*argument_token_range));

    let Ok(expansion_id) = model.immediate_macro_expansion(call.id) else {
        panic!("function-like macro call should have an immediate expansion");
    };
    let expansion = model.macro_expansions().get(expansion_id).unwrap();
    assert_eq!(expansion.emitted_token_range.start, emitted.id);
    assert_eq!(expansion.emitted_token_range.len, 1);
}

#[test]
fn source_model_maps_nested_macro_usage_in_actual_argument_to_source_spelling() {
    let root_text = r#"`define PAYL payload_i
`define NEXT(x) ((x) + 12'd1)
module m(input logic [3:0] payload_i, output logic [3:0] y);
assign y = `NEXT(`PAYL);
endmodule
"#;
    let (model, root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    let next_usage_index = model
        .usages()
        .iter()
        .position(|usage| usage.name.as_deref() == Some("NEXT"))
        .expect("outer function macro usage should be traced");
    let next_usage = &model.usages()[next_usage_index];
    assert_eq!(next_usage.arguments.len(), 1);
    let next_argument_range = next_usage.arguments[0]
        .argument_range
        .expect("actual argument should keep written source range");
    assert_eq!(next_argument_range.source, root_source);
    assert_eq!(text_at_range(root_text, next_argument_range.range), "`PAYL");
    assert_eq!(
        next_usage.arguments[0].tokens.iter().map(|token| token.raw.as_str()).collect::<Vec<_>>(),
        vec!["`PAYL"]
    );

    let next_reference = reference_for_usage(&model, next_usage_index);
    let next_call = model
        .macro_calls()
        .iter()
        .find(|call| call.reference == next_reference.id)
        .expect("outer macro usage should create a call");
    assert_eq!(next_call.arguments[0].argument_range, Some(next_argument_range));
    let Ok(next_expansion_id) = model.immediate_macro_expansion(next_call.id) else {
        panic!("outer macro usage should have an immediate expansion");
    };

    let payl_usage_index = model
        .usages()
        .iter()
        .position(|usage| usage.name.as_deref() == Some("PAYL"))
        .expect("nested actual-argument macro usage should be traced");
    let payl_usage = &model.usages()[payl_usage_index];
    assert_eq!(payl_usage.range.source, root_source);
    assert_eq!(text_at_range(root_text, payl_usage.range.range), "`PAYL");
    let payl_reference = reference_for_usage(&model, payl_usage_index);
    let SourceMacroResolution::Resolved { definition, .. } = &payl_reference.resolution else {
        panic!("PAYL usage should resolve through its runtime definition trace id");
    };
    assert_eq!(model.macro_definitions().get(*definition).unwrap().name.as_str(), "PAYL");
    let payl_call = model
        .macro_calls()
        .iter()
        .find(|call| call.reference == payl_reference.id)
        .expect("nested PAYL usage should create a call");
    assert_eq!(payl_call.parent_trace_expansion, next_call.trace_expansion);

    let Ok(payl_expansion_id) = model.immediate_macro_expansion(payl_call.id) else {
        panic!("nested PAYL usage should have its own immediate expansion");
    };
    let payl_expansion = model.macro_expansions().get(payl_expansion_id).unwrap();
    assert_eq!(payl_expansion.call, payl_call.id);

    let (payload, payload_trace_call, payload_body_range) = model
        .emitted_tokens()
        .iter()
        .find_map(|token| {
            let SourceTokenOrigin::MacroBody { trace_call, call, body_token_range, .. } =
                model.token_origins().get(token.origin?)?
            else {
                return None;
            };
            (*call == payl_call.id).then_some((token, *trace_call, *body_token_range))
        })
        .expect("PAYL emitted token should keep direct macro body origin");
    assert_eq!(payload.text.as_str(), "payload_i");
    assert_eq!(text_at_range(root_text, payload_body_range.range), "payload_i");
    assert_eq!(Some(payload_trace_call), payl_call.trace_call);
    assert_eq!(payl_call.parent_trace_expansion, next_call.trace_expansion);
    assert_eq!(payl_expansion.emitted_token_range.start, payload.id);
    assert_eq!(payl_expansion.emitted_token_range.len, 1);

    let next_expansion = model.macro_expansions().get(next_expansion_id).unwrap();
    assert!(next_expansion.child_calls.contains(&payl_call.id));
    assert!(payl_expansion.child_calls.is_empty());
}
