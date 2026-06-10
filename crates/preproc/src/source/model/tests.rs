use smol_str::SmolStr;
use syntax::{
    PreprocessorTrace, PreprocessorTraceEvent, PreprocessorTraceEventId,
    PreprocessorTraceMacroBodyIdentity, PreprocessorTraceMacroCallId,
    PreprocessorTraceMacroDefinitionId, PreprocessorTraceMacroExpansionId, PreprocessorTraceToken,
    PreprocessorTraceTokenProvenance, SourceBufferId, SourceBufferOrigin, SourceBufferRange,
    SyntaxKind, SyntaxTree, SyntaxTreeBuffer, SyntaxTreeOptions, TokenKind,
};
use utils::line_index::{TextRange, TextSize};

use super::{super::SourceMacroReferenceSite, *};

const ROOT_PATH: &str = "sample/rtl/top.sv";
const HEADER_PATH: &str = "sample/include/defs.vh";
const INCLUDE_DIR: &str = "sample/include";

fn preprocessor_trace(
    root_text: &str,
    name: &str,
    path: &str,
    options: &SyntaxTreeOptions,
) -> PreprocessorTrace {
    SyntaxTree::from_text_with_options_and_trace(root_text, name, path, options)
        .preprocessor_trace
        .expect("parse-derived trace should be present when requested")
}

fn source_model(
    root_text: &str,
    header_text: &str,
) -> (SourcePreprocModel, PreprocSourceId, PreprocSourceId) {
    let options = SyntaxTreeOptions {
        include_paths: vec![INCLUDE_DIR.to_owned()],
        include_buffers: vec![SyntaxTreeBuffer {
            path: HEADER_PATH.to_owned(),
            text: header_text.to_owned(),
        }],
        expand_includes: true,
        ..SyntaxTreeOptions::default()
    };
    let trace = preprocessor_trace(root_text, "source", ROOT_PATH, &options);
    let root_source = PreprocSourceId::from(trace.root_buffer_id);
    let model = SourcePreprocModel::from_trace(trace).unwrap();
    let header_source = source_by_path_suffix(&model, "defs.vh");
    (model, root_source, header_source)
}

fn source_by_path_suffix(model: &SourcePreprocModel, suffix: &str) -> PreprocSourceId {
    model
        .sources()
        .iter()
        .find(|source| {
            matches!(source.origin, PreprocSourceOrigin::Included { .. })
                && source.path.as_str().replace('\\', "/").ends_with(suffix)
        })
        .unwrap_or_else(|| panic!("source ending with {suffix} should be present"))
        .id
}

fn source_model_from_root(
    root_text: &str,
    options: SyntaxTreeOptions,
) -> (SourcePreprocModel, PreprocSourceId) {
    let trace = preprocessor_trace(root_text, "source", ROOT_PATH, &options);
    let root_source = PreprocSourceId::from(trace.root_buffer_id);
    let model = SourcePreprocModel::from_trace(trace).unwrap();
    (model, root_source)
}

fn offset_before(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).unwrap()).unwrap())
}

fn offset_after(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).unwrap() + needle.len()).unwrap())
}

fn text_at_range(text: &str, range: TextRange) -> &str {
    &text[usize::from(range.start())..usize::from(range.end())]
}

fn source_range(source: PreprocSourceId, start: u32, end: u32) -> SourceRange {
    SourceRange { source, range: TextRange::new(TextSize::from(start), TextSize::from(end)) }
}

fn visible_macro_names(
    model: &SourcePreprocModel,
    source: PreprocSourceId,
    offset: TextSize,
) -> Vec<SmolStr> {
    model
        .visible_macros_at(SourcePosition { source, offset })
        .into_iter()
        .map(|definition| definition.name.clone())
        .collect()
}

fn visible_macro_definition<'a>(
    model: &'a SourcePreprocModel,
    source: PreprocSourceId,
    offset: TextSize,
    name: &str,
) -> Option<&'a SourceMacroDefinition> {
    model
        .visible_macros_at(SourcePosition { source, offset })
        .into_iter()
        .find(|definition| definition.name == name)
}

fn reference_for_usage(model: &SourcePreprocModel, usage_index: usize) -> &SourceMacroReference {
    model
        .macro_references()
        .iter()
        .find(|reference| {
            matches!(
                reference.site,
                SourceMacroReferenceSite::Usage {
                    usage_index: site_usage_index,
                } if site_usage_index == usage_index
            )
        })
        .expect("usage reference should be in resolved reference table")
}

fn reference_for_conditional_token(
    model: &SourcePreprocModel,
    conditional_index: usize,
    token_index: usize,
) -> &SourceMacroReference {
    model
        .macro_references()
        .iter()
        .find(|reference| {
            matches!(
                reference.site,
                SourceMacroReferenceSite::ConditionalToken {
                    conditional_index: site_conditional_index,
                    token_index: site_token_index,
                } | SourceMacroReferenceSite::IncludeGuardIfNDef {
                    conditional_index: site_conditional_index,
                    token_index: site_token_index,
                } if site_conditional_index == conditional_index
                    && site_token_index == token_index
            )
        })
        .expect("conditional token reference should be in resolved reference table")
}

#[test]
fn source_model_applies_include_define_after_include_point_only() {
    let root_text = r#"`include "defs.vh"
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    assert!(
        !visible_macro_names(&model, root_source, offset_before(root_text, "`include"))
            .iter()
            .any(|name| name == "HEADER_WIDTH")
    );

    let after_include = visible_macro_definition(
        &model,
        root_source,
        offset_after(root_text, "`include \"defs.vh\"\n"),
        "HEADER_WIDTH",
    )
    .unwrap();
    assert_eq!(after_include.id.raw(), 0);

    let definition = model
        .visible_macros_at(SourcePosition {
            source: root_source,
            offset: offset_after(root_text, "`include \"defs.vh\"\n"),
        })
        .into_iter()
        .find(|definition| definition.name == "HEADER_WIDTH")
        .unwrap();
    assert_eq!(definition.name_range.source, header_source);
}

#[test]
fn source_model_undef_removes_included_define() {
    let root_text = r#"`include "defs.vh"
`undef HEADER_WIDTH
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let after_include = visible_macro_definition(
        &model,
        root_source,
        offset_after(root_text, "`include \"defs.vh\"\n"),
        "HEADER_WIDTH",
    )
    .unwrap();
    assert_eq!(after_include.id.raw(), 0);
    assert_eq!(model.defines()[0].name_range.unwrap().source, header_source);

    assert!(
        visible_macro_definition(
            &model,
            root_source,
            offset_after(root_text, "`undef HEADER_WIDTH\n"),
            "HEADER_WIDTH",
        )
        .is_none()
    );
    assert_eq!(model.undefs()[0].name.as_deref(), Some("HEADER_WIDTH"));
    assert_eq!(model.undefs()[0].name_range.unwrap().source, root_source);
}

#[test]
fn source_model_same_name_define_overrides_included_define() {
    let root_text = r#"`include "defs.vh"
`define HEADER_WIDTH 16
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    assert_eq!(model.defines()[0].name_range.unwrap().source, header_source);
    assert_eq!(model.defines()[1].name_range.unwrap().source, root_source);

    let after_override = visible_macro_definition(
        &model,
        root_source,
        offset_after(root_text, "`define HEADER_WIDTH 16\n"),
        "HEADER_WIDTH",
    )
    .unwrap();
    assert_eq!(after_override.id.raw(), 1);

    let definition = model
        .visible_macros_at(SourcePosition {
            source: root_source,
            offset: offset_after(root_text, "`define HEADER_WIDTH 16\n"),
        })
        .into_iter()
        .find(|definition| definition.name == "HEADER_WIDTH")
        .unwrap();
    assert_eq!(definition.body_tokens[0].value.as_str(), "16");
    assert_eq!(definition.name_range.source, root_source);
}

#[test]
fn visible_macro_query_reads_timeline_without_event_records() {
    let root_text = r#"`define A 1
`undef A
`define B 2
"#;
    let trace = preprocessor_trace(root_text, "source", ROOT_PATH, &SyntaxTreeOptions::default());
    let root_source = PreprocSourceId::from(trace.root_buffer_id);
    let mut model = SourcePreprocModel::from_trace(trace).unwrap();

    let names_after_define =
        visible_macro_names(&model, root_source, offset_after(root_text, "`define A 1\n"));
    let names_after_undef =
        visible_macro_names(&model, root_source, offset_after(root_text, "`undef A\n"));
    let names_after_second_define =
        visible_macro_names(&model, root_source, offset_after(root_text, "`define B 2\n"));

    assert_eq!(names_after_define, vec![SmolStr::new("A")]);
    assert!(names_after_undef.is_empty(), "{names_after_undef:?}");
    assert_eq!(names_after_second_define, vec![SmolStr::new("B")]);

    model.index.event_records.clear();

    assert_eq!(
        visible_macro_names(&model, root_source, offset_after(root_text, "`define A 1\n")),
        names_after_define
    );
    assert_eq!(
        visible_macro_names(&model, root_source, offset_after(root_text, "`undef A\n")),
        names_after_undef
    );
    assert_eq!(
        visible_macro_names(&model, root_source, offset_after(root_text, "`define B 2\n")),
        names_after_second_define
    );
}

#[test]
fn included_plain_source_uses_include_scope_macro_state() {
    let root_text = r#"`define BEFORE 1
`include "defs.vh"
`define AFTER 1
"#;
    let header_text = "wire x;\n";
    let (model, _, header_source) = source_model(root_text, header_text);

    let names = visible_macro_names(&model, header_source, offset_after(header_text, "wire x"));

    assert!(names.iter().any(|name| name == "BEFORE"), "{names:?}");
    assert!(!names.iter().any(|name| name == "AFTER"), "{names:?}");
}

#[test]
fn included_source_after_last_directive_uses_include_scope_macro_state() {
    let root_text = r#"`define BEFORE 1
`include "defs.vh"
`define AFTER 1
"#;
    let header_text = "`define FROM_HEADER 1\nwire x;\n";
    let (model, _, header_source) = source_model(root_text, header_text);

    let names = visible_macro_names(&model, header_source, offset_after(header_text, "wire x"));

    assert!(names.iter().any(|name| name == "BEFORE"), "{names:?}");
    assert!(names.iter().any(|name| name == "FROM_HEADER"), "{names:?}");
    assert!(!names.iter().any(|name| name == "AFTER"), "{names:?}");
}

#[test]
fn source_model_preserves_inactive_range_sources() {
    let root_text = r#"`include "defs.vh"
`ifndef HEADER_FLAG
wire disabled_by_header;
`endif
"#;
    let header_text = r#"`define HEADER_FLAG
`ifdef NEVER
wire disabled_from_header;
`endif
"#;
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let root_inactive =
        model.inactive_ranges().iter().find(|range| range.source == root_source).unwrap();
    assert_eq!(text_at_range(root_text, root_inactive.range), "wire disabled_by_header;");

    let header_inactive =
        model.inactive_ranges().iter().find(|range| range.source == header_source).unwrap();
    assert_eq!(text_at_range(header_text, header_inactive.range), "wire disabled_from_header;");
}

#[test]
fn source_model_resolves_root_usage_to_included_define() {
    let root_text = r#"`include "defs.vh"
logic [`HEADER_WIDTH-1:0] data;
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let usage_index = model
        .usages()
        .iter()
        .position(|usage| usage.name.as_deref() == Some("HEADER_WIDTH"))
        .expect("root macro usage should be traced");
    let usage = &model.usages()[usage_index];
    assert_eq!(usage.range.source, root_source);
    assert_eq!(usage.name_range.unwrap().source, root_source);

    let reference = reference_for_usage(&model, usage_index);
    let SourceMacroResolution::Resolved { definition, include_chain, reason } =
        &reference.resolution
    else {
        panic!("usage reference should resolve to included definition");
    };
    assert_eq!(*reason, SourceMacroResolutionReason::VisibleDefinition);
    let definition = model.macro_definitions().get(*definition).unwrap();
    assert_eq!(definition.name.as_str(), "HEADER_WIDTH");
    assert_eq!(definition.name_range.source, header_source);
    assert_eq!(definition.body_tokens[0].value.as_str(), "8");
    assert_eq!(include_chain.len(), 1);
    assert_eq!(include_chain[0].include_range.source, root_source);
    assert_eq!(include_chain[0].included_source, header_source);
}

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
                model.token_provenance().get(token.provenance)?
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
fn source_model_uses_direct_definition_identity_when_body_ranges_collide() {
    let trace = PreprocessorTrace {
        root_buffer_id: 1,
        source_buffers: vec![SourceBufferId {
            path: ROOT_PATH.to_owned(),
            text: None,
            buffer_id: 1,
            origin: SourceBufferOrigin::Source,
        }],
        events: vec![
            PreprocessorTraceEvent {
                event_id: PreprocessorTraceEventId(0),
                kind: SyntaxKind::DEFINE_DIRECTIVE,
                range: Some(SourceBufferRange { buffer_id: 1, range: 0..12 }),
                macro_definition_id: Some(PreprocessorTraceMacroDefinitionId(10)),
                macro_call_id: None,
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(PreprocessorTraceToken {
                    raw_text: "A".to_owned(),
                    value_text: "A".to_owned(),
                    token_kind: TokenKind::IDENTIFIER,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..9 }),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: vec![PreprocessorTraceToken {
                    raw_text: "1".to_owned(),
                    value_text: "1".to_owned(),
                    token_kind: TokenKind::INTEGER_LITERAL,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..9 }),
                }],
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
            PreprocessorTraceEvent {
                event_id: PreprocessorTraceEventId(1),
                kind: SyntaxKind::DEFINE_DIRECTIVE,
                range: Some(SourceBufferRange { buffer_id: 1, range: 13..25 }),
                macro_definition_id: Some(PreprocessorTraceMacroDefinitionId(20)),
                macro_call_id: None,
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(PreprocessorTraceToken {
                    raw_text: "B".to_owned(),
                    value_text: "B".to_owned(),
                    token_kind: TokenKind::IDENTIFIER,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 21..22 }),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: vec![PreprocessorTraceToken {
                    raw_text: "2".to_owned(),
                    value_text: "2".to_owned(),
                    token_kind: TokenKind::INTEGER_LITERAL,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..9 }),
                }],
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
            PreprocessorTraceEvent {
                event_id: PreprocessorTraceEventId(2),
                kind: SyntaxKind::MACRO_USAGE,
                range: Some(SourceBufferRange { buffer_id: 1, range: 40..42 }),
                macro_definition_id: None,
                macro_call_id: Some(PreprocessorTraceMacroCallId(200)),
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(PreprocessorTraceToken {
                    raw_text: "`B".to_owned(),
                    value_text: "`B".to_owned(),
                    token_kind: TokenKind::DIRECTIVE,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 40..42 }),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: Vec::new(),
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
        ],
        include_edges: Vec::new(),
        emitted_tokens: vec![syntax::PreprocessorTraceEmittedToken {
            raw_text: "2".to_owned(),
            value_text: "2".to_owned(),
            display_text: "2".to_owned(),
            token_kind: TokenKind::INTEGER_LITERAL,
            provenance: PreprocessorTraceTokenProvenance::MacroBody {
                macro_name: "B".to_owned(),
                identity: PreprocessorTraceMacroBodyIdentity {
                    call_id: PreprocessorTraceMacroCallId(200),
                    definition_id: PreprocessorTraceMacroDefinitionId(20),
                    expansion_id: PreprocessorTraceMacroExpansionId(300),
                    parent_expansion_id: None,
                    body_token_index: 0,
                },
                call_range: SourceBufferRange { buffer_id: 1, range: 40..42 },
                body_token_range: SourceBufferRange { buffer_id: 1, range: 8..9 },
            },
        }],
    };
    let model = SourcePreprocModel::from_trace(trace).unwrap();
    let emitted = model.emitted_tokens().iter().find(|token| token.text == "2").unwrap();
    let SourceTokenProvenance::MacroBody { definition, call, identity, .. } =
        model.token_provenance().get(emitted.provenance).unwrap()
    else {
        panic!("colliding range token should still resolve through direct body identity");
    };

    let definition = model.macro_definitions().get(*definition).unwrap();
    assert_eq!(definition.name.as_str(), "B");
    assert_eq!(definition.identity, Some(identity.definition));
    assert_eq!(model.macro_calls().get(*call).unwrap().identity, Some(identity.call));
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
            } = model.token_provenance().get(token.provenance)?
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
            } = model.token_provenance().get(token.provenance)?
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

#[test]
fn source_model_marks_missing_direct_identity_partial_without_range_fallback() {
    let root_source = PreprocSourceId::from(1);
    let define_range = source_range(root_source, 0, 11);
    let name_range = source_range(root_source, 8, 9);
    let body_range = source_range(root_source, 10, 11);
    let usage_range = source_range(root_source, 24, 26);
    let index = SourcePreprocIndex {
        root_source: Some(root_source),
        sources: vec![PreprocSource {
            id: root_source,
            path: SmolStr::new(ROOT_PATH),
            origin: PreprocSourceOrigin::Root,
        }],
        event_records: vec![
            SourcePreprocEventRecord {
                event_id: SourcePreprocEventId(0),
                kind: MacroEventKind::Define,
                range: define_range,
                index: 0,
            },
            SourcePreprocEventRecord {
                event_id: SourcePreprocEventId(1),
                kind: MacroEventKind::Usage,
                range: usage_range,
                index: 0,
            },
        ],
        emitted_tokens: vec![SourceEmittedTokenFact {
            raw: SmolStr::new("1"),
            value: SmolStr::new("1"),
            display: SmolStr::new("1"),
            kind: SourceTokenKind::Syntax(TokenKind::INTEGER_LITERAL),
            provenance: SourceTokenProvenanceFact::MacroBody {
                macro_name: SmolStr::new("A"),
                identity: None,
                call_range: usage_range,
                body_token_range: body_range,
            },
        }],
        defines: vec![SourceMacroDefine {
            event_id: SourcePreprocEventId(0),
            identity: Some(SourceMacroDefinitionKey::new(10)),
            name: Some(SmolStr::new("A")),
            name_range: Some(name_range),
            params: None,
            body: vec![SourceMacroToken {
                raw: SmolStr::new("1"),
                value: SmolStr::new("1"),
                range: Some(body_range),
            }],
            range: define_range,
        }],
        usages: vec![SourceMacroUsage {
            event_id: SourcePreprocEventId(1),
            identity: Some(SourceMacroCallKey::new(20)),
            definition_identity: None,
            expansion_identity: None,
            parent_expansion_identity: None,
            name: Some(SmolStr::new("A")),
            name_range: Some(usage_range),
            arguments: Vec::new(),
            range: usage_range,
        }],
        ..SourcePreprocIndex::default()
    };

    let model = SourcePreprocModel::new(index);
    let emitted = model.emitted_tokens().iter().next().unwrap();
    assert!(matches!(
        model.token_provenance().get(emitted.provenance).unwrap(),
        SourceTokenProvenance::Unavailable(
            SourcePreprocUnavailable::MissingEmittedTokenMacroCallIdentity
        )
    ));
    assert_eq!(model.capabilities().emitted_token_provenance, CapabilityStatus::Partial);
    assert_eq!(model.capabilities().macro_expansions, CapabilityStatus::Partial);
}

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
        model.token_provenance().get(emitted.provenance).unwrap()
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
    let SourceTokenProvenance::TokenPaste {
        call: paste_call,
        identity: paste_identity,
        inputs: paste_inputs,
    } = model.token_provenance().get(pasted.provenance).unwrap()
    else {
        panic!(
            "token paste should carry macro operation provenance: {:?}",
            model.token_provenance().get(pasted.provenance).unwrap()
        );
    };
    assert!(!paste_inputs.is_empty());
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
        inputs: stringification_inputs,
    } = model.token_provenance().get(stringified.provenance).unwrap()
    else {
        panic!("stringification should carry macro operation provenance");
    };
    assert!(!stringification_inputs.is_empty());
    assert_eq!(
        Some(stringification_identity.call),
        model.macro_calls().get(*stringification_call).and_then(|call| call.identity)
    );
    assert_ne!(paste_call, stringification_call);
    assert_eq!(model.capabilities().emitted_tokens, CapabilityStatus::Complete);
    assert_eq!(model.capabilities().emitted_token_provenance, CapabilityStatus::Complete);
    assert_eq!(model.capabilities().macro_expansions, CapabilityStatus::Complete);
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

#[test]
fn source_model_does_not_create_expansion_without_emitted_token_authority() {
    let root_text = "`define A 1\nmodule m; localparam int W = `A; endmodule\n";
    let define_start = root_text.find("`define").unwrap();
    let define_end = root_text.find('\n').unwrap();
    let usage_start = root_text.find("`A").unwrap();
    let trace = PreprocessorTrace {
        root_buffer_id: 1,
        source_buffers: vec![SourceBufferId {
            path: ROOT_PATH.to_owned(),
            text: None,
            buffer_id: 1,
            origin: SourceBufferOrigin::Source,
        }],
        events: vec![
            PreprocessorTraceEvent {
                event_id: PreprocessorTraceEventId(0),
                kind: SyntaxKind::DEFINE_DIRECTIVE,
                range: Some(SourceBufferRange { buffer_id: 1, range: define_start..define_end }),
                macro_definition_id: None,
                macro_call_id: None,
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(PreprocessorTraceToken {
                    raw_text: "A".to_owned(),
                    value_text: "A".to_owned(),
                    token_kind: TokenKind::IDENTIFIER,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..9 }),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: vec![PreprocessorTraceToken {
                    raw_text: "1".to_owned(),
                    value_text: "1".to_owned(),
                    token_kind: TokenKind::INTEGER_LITERAL,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 10..11 }),
                }],
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
            PreprocessorTraceEvent {
                event_id: PreprocessorTraceEventId(1),
                kind: SyntaxKind::MACRO_USAGE,
                range: Some(SourceBufferRange {
                    buffer_id: 1,
                    range: usage_start..usage_start + 2,
                }),
                macro_definition_id: None,
                macro_call_id: None,
                macro_expansion_id: None,
                parent_macro_expansion_id: None,
                directive: None,
                name: Some(PreprocessorTraceToken {
                    raw_text: "`A".to_owned(),
                    value_text: "`A".to_owned(),
                    token_kind: TokenKind::DIRECTIVE,
                    range: Some(SourceBufferRange {
                        buffer_id: 1,
                        range: usage_start..usage_start + 2,
                    }),
                }),
                include_file_name: None,
                params: Vec::new(),
                arguments: Vec::new(),
                body_tokens: Vec::new(),
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            },
        ],
        include_edges: Vec::new(),
        emitted_tokens: Vec::new(),
    };
    let model = SourcePreprocModel::from_trace(trace).unwrap();
    let call = model.macro_calls().iter().next().expect("usage should create a call");

    assert!(model.macro_expansions().is_empty());
    assert!(matches!(
        model.immediate_macro_expansion(call.id),
        SourceMacroExpansionQuery::Unavailable(
            SourcePreprocUnavailable::MissingEmittedTokenMacroExpansionIdentity { .. }
        )
    ));
    assert_eq!(model.capabilities().macro_expansions, CapabilityStatus::Partial);
}

#[test]
fn source_model_keeps_zero_token_macro_expansion_available() {
    let root_text = r#"`define EMPTY
`define DROP(x)
module top;
`EMPTY
`DROP(foo)
endmodule
"#;
    let (model, _root_source) = source_model_from_root(root_text, SyntaxTreeOptions::default());

    for name in ["EMPTY", "DROP"] {
        let call = model
            .macro_calls()
            .iter()
            .find(|call| {
                model
                    .macro_references()
                    .get(call.reference)
                    .is_some_and(|reference| reference.name.as_str() == name)
            })
            .unwrap_or_else(|| panic!("{name} call should be traced"));
        let SourceMacroExpansionQuery::Available(expansion_id) =
            model.immediate_macro_expansion(call.id)
        else {
            panic!("{name} zero-token expansion should be available: {call:?}");
        };
        let expansion = model.macro_expansions().get(expansion_id).unwrap();
        assert_eq!(expansion.emitted_token_range.len, 0);
        assert_eq!(expansion.call, call.id);
        assert_eq!(expansion.status, SourceMacroExpansionStatus::Complete);
    }
    assert_eq!(model.capabilities().macro_expansions, CapabilityStatus::Complete);
}

#[test]
fn source_model_maps_predefine_and_intrinsic_provenance() {
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
    let SourceTokenProvenance::Predefine { source } =
        model.token_provenance().get(predefine.provenance).unwrap()
    else {
        panic!("configured predefine token should map to Predefine provenance");
    };
    assert!(model.sources().iter().any(|candidate| {
        candidate.id == *source && candidate.origin == PreprocSourceOrigin::Predefine
    }));

    let intrinsic = model
        .emitted_tokens()
        .iter()
        .find(|token| token.text.as_str() == "3")
        .expect("intrinsic macro token should stay in emitted stream");
    let SourceTokenProvenance::Builtin { name, call, identity } =
        model.token_provenance().get(intrinsic.provenance).unwrap()
    else {
        panic!("intrinsic macro token should have builtin provenance");
    };
    assert_eq!(name.as_str(), "__LINE__");
    assert_ne!(identity.call.raw(), 0);
    assert_ne!(identity.expansion.raw(), 0);

    let call = model.macro_calls().get(*call).expect("builtin provenance should map to a call");
    let SourceMacroExpansionQuery::Available(expansion_id) =
        model.immediate_macro_expansion(call.id)
    else {
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
    assert_eq!(decl_call.status, SourceMacroCallStatus::ExpansionAvailable);

    let SourceMacroExpansionQuery::Available(expansion_id) =
        model.immediate_macro_expansion(decl_call.id)
    else {
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
    assert_eq!(model.capabilities().macro_expansions, CapabilityStatus::Complete);
    assert_eq!(model.capabilities().emitted_token_provenance, CapabilityStatus::Complete);
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
    assert_eq!(pipe_call.status, SourceMacroCallStatus::ExpansionAvailable);

    let SourceMacroExpansionQuery::Available(expansion_id) =
        model.immediate_macro_expansion(pipe_call.id)
    else {
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
    assert_eq!(model.capabilities().macro_expansions, CapabilityStatus::Complete);
    assert_eq!(model.capabilities().emitted_token_provenance, CapabilityStatus::Complete);
}

#[test]
fn source_model_resolves_conditional_tokens_to_visible_defines() {
    let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
wire active;
`endif
"#;
    let header_text = "`define HEADER_FLAG\n";
    let (model, root_source, header_source) = source_model(root_text, header_text);

    let conditional_index = model
        .conditionals()
        .iter()
        .position(|conditional| conditional.kind == MacroConditionalKind::IfDef)
        .expect("ifdef should be traced");
    let reference = reference_for_conditional_token(&model, conditional_index, 0);

    assert_eq!(reference.name.as_str(), "HEADER_FLAG");
    assert_eq!(reference.name_range.source, root_source);
    let SourceMacroResolution::Resolved { definition, reason, .. } = reference.resolution else {
        panic!("conditional token reference should resolve to visible definition");
    };
    assert_eq!(reason, SourceMacroResolutionReason::VisibleDefinition);
    assert_eq!(model.macro_definitions().get(definition).unwrap().name_range.source, header_source);
}

#[test]
fn source_model_resolves_ifndef_include_guard_to_following_define() {
    let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
wire active;
`endif
"#;
    let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
    let (model, _root_source, header_source) = source_model(root_text, header_text);

    let conditional_index = model
        .conditionals()
        .iter()
        .position(|conditional| {
            conditional.kind == MacroConditionalKind::IfNDef
                && conditional.range.source == header_source
        })
        .expect("ifndef guard should be traced");
    let reference = model
        .macro_references()
        .iter()
        .find(|reference| {
            matches!(
                reference.site,
                SourceMacroReferenceSite::IncludeGuardIfNDef {
                    conditional_index: site_conditional_index,
                    token_index: 0,
                } if site_conditional_index == conditional_index
            )
        })
        .expect("include guard token should be modeled as a resolved reference");
    assert_eq!(reference.name.as_str(), "HEADER_FLAG");
    assert_eq!(reference.name_range.source, header_source);
    assert!(matches!(
        reference.resolution,
        SourceMacroResolution::Resolved {
            reason: SourceMacroResolutionReason::IncludeGuardIfNDef,
            ..
        }
    ));
}

#[test]
fn source_model_nested_include_resolution_carries_definition_chain() {
    let root_text = r#"`include "defs.vh"
logic [`LEAF_WIDTH-1:0] data;
"#;
    let header_text = "`include \"leaf.vh\"\n";
    let leaf_path = "sample/include/leaf.vh";
    let options = SyntaxTreeOptions {
        include_paths: vec![INCLUDE_DIR.to_owned()],
        include_buffers: vec![
            SyntaxTreeBuffer { path: HEADER_PATH.to_owned(), text: header_text.to_owned() },
            SyntaxTreeBuffer {
                path: leaf_path.to_owned(),
                text: "`define LEAF_WIDTH 4\n".to_owned(),
            },
        ],
        expand_includes: true,
        ..SyntaxTreeOptions::default()
    };
    let trace = preprocessor_trace(root_text, "source", ROOT_PATH, &options);
    let root_source = PreprocSourceId::from(trace.root_buffer_id);
    let model = SourcePreprocModel::from_trace(trace).unwrap();
    let header_source = source_by_path_suffix(&model, "include/defs.vh");
    let leaf_source = source_by_path_suffix(&model, "include/leaf.vh");

    let usage_index = model
        .usages()
        .iter()
        .position(|usage| usage.name.as_deref() == Some("LEAF_WIDTH"))
        .expect("root macro usage should be traced");
    let reference = reference_for_usage(&model, usage_index);
    let SourceMacroResolution::Resolved { definition, include_chain, .. } = &reference.resolution
    else {
        panic!("usage reference should resolve to nested included definition");
    };

    assert_eq!(model.macro_definitions().get(*definition).unwrap().name_range.source, leaf_source);
    assert_eq!(include_chain.len(), 2);
    assert_eq!(include_chain[0].include_range.source, root_source);
    assert_eq!(include_chain[0].included_source, header_source);
    assert_eq!(include_chain[1].include_range.source, header_source);
    assert_eq!(include_chain[1].included_source, leaf_source);
}

#[test]
fn source_model_fails_closed_when_directive_event_range_is_missing() {
    let trace = PreprocessorTrace {
        root_buffer_id: 1,
        source_buffers: vec![SourceBufferId {
            path: ROOT_PATH.to_owned(),
            text: None,
            buffer_id: 1,
            origin: SourceBufferOrigin::Source,
        }],
        events: vec![PreprocessorTraceEvent {
            event_id: PreprocessorTraceEventId(0),
            kind: SyntaxKind::DEFINE_DIRECTIVE,
            range: None,
            macro_definition_id: None,
            macro_call_id: None,
            macro_expansion_id: None,
            parent_macro_expansion_id: None,
            directive: None,
            name: Some(PreprocessorTraceToken {
                raw_text: "WIDTH".to_owned(),
                value_text: "WIDTH".to_owned(),
                token_kind: TokenKind::IDENTIFIER,
                range: Some(SourceBufferRange { buffer_id: 1, range: 8..13 }),
            }),
            include_file_name: None,
            params: Vec::new(),
            arguments: Vec::new(),
            body_tokens: Vec::new(),
            expr_tokens: Vec::new(),
            disabled_ranges: Vec::new(),
        }],
        include_edges: Vec::new(),
        emitted_tokens: Vec::new(),
    };

    assert_eq!(
        SourcePreprocModel::from_trace(trace).unwrap_err(),
        SourcePreprocError::MissingEventRange { source_order: 0, kind: MacroEventKind::Define }
    );
}
