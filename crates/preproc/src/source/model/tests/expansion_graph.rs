use super::*;

#[test]
fn source_model_exposes_expansion_provenance_skeleton_tables() {
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
        .expect("macro usage should create a call fact");
    assert_eq!(call.call_range.source, root_source);
    assert_eq!(call.status, SourceMacroCallStatus::ExpansionAvailable);
    let SourceMacroExpansionQuery::Available(expansion_id) =
        model.immediate_macro_expansion(call.id)
    else {
        panic!("object-like macro call should have an immediate expansion");
    };
    assert_eq!(call.expansion, Some(expansion_id));
    let expansion = model.macro_expansions().get(expansion_id).unwrap();
    assert_eq!(expansion.call, call.id);
    assert_eq!(expansion.definition, SourceMacroExpansionDefinition::Source(*resolved_definition));
    assert!(expansion.child_calls.is_empty());
    assert_eq!(expansion.status, SourceMacroExpansionStatus::Complete);

    let emitted = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "8")
        .expect("macro body token should be emitted by adapter authority");
    assert_eq!(expansion.emitted_token_range.start, emitted.id);
    assert_eq!(expansion.emitted_token_range.len, 1);
    let provenance = model.token_provenance().get(emitted.provenance).unwrap();
    assert!(matches!(
        provenance,
        SourceTokenProvenance::MacroBody {
            definition: body_definition,
            body_token_range,
            call: body_call,
            ..
        } if *body_definition == *resolved_definition
            && body_token_range.source == header_source
            && *body_call == call.id
    ));
    let recursive = model.recursive_macro_expansion(call.id);
    assert_eq!(recursive.expansions, vec![expansion_id]);
    assert!(recursive.unavailable.is_empty());
    assert_eq!(model.capabilities().macro_calls, CapabilityStatus::Complete);
    assert_eq!(model.capabilities().macro_expansions, CapabilityStatus::Complete);
    assert_eq!(model.capabilities().emitted_tokens, CapabilityStatus::Complete);
    assert_eq!(model.capabilities().emitted_token_provenance, CapabilityStatus::Complete);
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
    let SourceTokenProvenance::MacroArgument { call, argument_index, argument_token_range, .. } =
        model.token_provenance().get(emitted.provenance).unwrap()
    else {
        panic!("argument replacement should map to MacroArgument provenance");
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

    let SourceMacroExpansionQuery::Available(expansion_id) =
        model.immediate_macro_expansion(call.id)
    else {
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
    assert!(matches!(
        model.immediate_macro_expansion(next_call.id),
        SourceMacroExpansionQuery::Available(_)
    ));

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
        panic!("PAYL usage should resolve through its runtime definition identity");
    };
    assert_eq!(model.macro_definitions().get(*definition).unwrap().name.as_str(), "PAYL");
    let payl_call = model
        .macro_calls()
        .iter()
        .find(|call| call.reference == payl_reference.id)
        .expect("nested PAYL usage should create a call");
    assert_eq!(payl_call.parent_expansion_identity, next_call.expansion_identity);

    let SourceMacroExpansionQuery::Available(payl_expansion_id) =
        model.immediate_macro_expansion(payl_call.id)
    else {
        panic!("nested PAYL usage should have its own immediate expansion");
    };
    let payl_expansion = model.macro_expansions().get(payl_expansion_id).unwrap();
    assert_eq!(payl_expansion.call, payl_call.id);

    let (payload, payload_identity, payload_body_range) = model
        .emitted_tokens()
        .iter()
        .find_map(|token| {
            let SourceTokenProvenance::MacroBody { identity, call, body_token_range, .. } =
                model.token_provenance().get(token.provenance)?
            else {
                return None;
            };
            (*call == payl_call.id).then_some((token, *identity, *body_token_range))
        })
        .expect("PAYL emitted token should keep direct macro body provenance");
    assert_eq!(payload.text.as_str(), "payload_i");
    assert_eq!(text_at_range(root_text, payload_body_range.range), "payload_i");
    assert_eq!(Some(payload_identity.call), payl_call.identity);
    assert_eq!(Some(payload_identity.expansion), payl_call.expansion_identity);
    assert_eq!(payload_identity.parent_expansion, next_call.expansion_identity);
    assert_eq!(payl_expansion.emitted_token_range.start, payload.id);
    assert_eq!(payl_expansion.emitted_token_range.len, 1);

    let recursive = model.recursive_macro_expansion(next_call.id);
    assert!(recursive.expansions.contains(&payl_expansion_id));
    assert!(recursive.unavailable.is_empty());
}
