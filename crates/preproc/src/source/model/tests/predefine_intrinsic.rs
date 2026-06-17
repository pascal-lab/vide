use super::*;

#[test]
fn source_model_maps_predefine_and_intrinsic_origin() {
    let root_text = r#"module m;
localparam int P = `FROM_API;
localparam int L = `__LINE__;
endmodule
"#;
    let (model, _root_source) = source_model_from_root(
        root_text,
        SyntaxTreeOptions {
            predefines: vec!["FROM_API=11".to_owned()],
            ..SyntaxTreeOptions::default()
        },
    );

    let predefine = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "11")
        .expect("predefine expansion token should be emitted");
    let SourceTokenOrigin::Predefine { source } =
        model.token_origins().get(predefine.origin.unwrap()).unwrap()
    else {
        panic!("configured predefine token should map to Predefine origin");
    };
    assert!(model.sources().iter().any(|candidate| {
        candidate.id == *source && candidate.origin == PreprocSourceOrigin::Predefine
    }));

    let intrinsic = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "3")
        .expect("intrinsic macro token should stay in emitted stream");
    let SourceTokenOrigin::Builtin { name, call, trace_call, trace_expansion, .. } =
        model.token_origins().get(intrinsic.origin.unwrap()).unwrap()
    else {
        panic!("intrinsic macro token should have builtin origin");
    };
    assert_eq!(name.as_str(), "__LINE__");
    assert_ne!(trace_call.0, 0);
    assert_ne!(trace_expansion.0, 0);

    let call = model.macro_calls().get(*call).expect("builtin origin should map to a call");
    let Ok(expansion_id) = model.immediate_macro_expansion(call.id) else {
        panic!("builtin macro call should have an immediate expansion");
    };
    let expansion = model.macro_expansions().get(expansion_id).unwrap();
    assert_eq!(
        expansion.definition,
        SourceMacroExpansionDefinition::Builtin { name: "__LINE__".into() }
    );
}

#[test]
fn source_model_keeps_macro_expansion_contiguous_across_predefine_tokens() {
    let root_text = r#"`define DECL_PIPE(name, width) logic [(width)-1:0] name``_q
module m;
  `DECL_PIPE(sample, `LANE_WIDTH);
endmodule
"#;
    let (model, _root_source) = source_model_from_root(
        root_text,
        SyntaxTreeOptions {
            predefines: vec!["LANE_WIDTH=12".to_owned()],
            ..SyntaxTreeOptions::default()
        },
    );

    let decl_call = model
        .macro_calls()
        .iter()
        .find(|call| {
            model
                .macro_references()
                .get(call.reference)
                .is_some_and(|reference| reference.name.as_str() == "DECL_PIPE")
        })
        .expect("DECL_PIPE call should be traced");

    let Ok(expansion_id) = model.immediate_macro_expansion(decl_call.id) else {
        panic!("DECL_PIPE call should have a complete expansion");
    };
    let expansion = model.macro_expansions().get(expansion_id).unwrap();
    let start = expansion.emitted_token_range.start.raw();
    let end = start + expansion.emitted_token_range.len;
    let expanded = (start..end)
        .filter_map(|raw| model.emitted_tokens().get(SourceEmittedTokenId::new(raw)))
        .map(|token| token.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        expanded.contains("logic [ ( 12 ) - 1 : 0 ] sample_q"),
        "predefine token should stay inside the parent expansion stream: {expanded}"
    );
}

#[test]
fn source_model_keeps_macro_actual_argument_expansion_contiguous_across_predefine_tokens() {
    let root_text = r#"`define PIPE_ASSIGN(name, next_value) \
  always_ff @(posedge clk_i or negedge rst_ni) begin \
    if (!rst_ni) begin \
      name``_q <= '0; \
    end else begin \
      name``_q <= (next_value); \
    end \
  end
module m;
  `PIPE_ASSIGN(trace, sample_q ^ {{(`LANE_WIDTH-1){1'b0}}, 1'b1});
endmodule
"#;
    let (model, _root_source) = source_model_from_root(
        root_text,
        SyntaxTreeOptions {
            predefines: vec!["LANE_WIDTH=12".to_owned()],
            ..SyntaxTreeOptions::default()
        },
    );

    let pipe_call = model
        .macro_calls()
        .iter()
        .find(|call| {
            model
                .macro_references()
                .get(call.reference)
                .is_some_and(|reference| reference.name.as_str() == "PIPE_ASSIGN")
        })
        .expect("PIPE_ASSIGN call should be traced");

    let Ok(expansion_id) = model.immediate_macro_expansion(pipe_call.id) else {
        panic!("PIPE_ASSIGN call should have a complete expansion");
    };
    let expansion = model.macro_expansions().get(expansion_id).unwrap();
    let start = expansion.emitted_token_range.start.raw();
    let end = start + expansion.emitted_token_range.len;
    let expanded = (start..end)
        .filter_map(|raw| model.emitted_tokens().get(SourceEmittedTokenId::new(raw)))
        .map(|token| token.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        expanded.contains("trace_q <= ( sample_q ^ { { ( 12 - 1 ) { 1 'b 0 } } , 1 'b 1 } )"),
        "predefine token and following argument tokens should stay inside the parent expansion stream: {expanded}"
    );
}
