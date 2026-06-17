use smol_str::SmolStr;
use syntax::{
    SourceBufferId, SourceBufferOrigin, SourceBufferRange, SyntaxKind, SyntaxTree,
    SyntaxTreeBuffer, SyntaxTreeOptions, TokenKind,
    preproc::{
        Event, EventId, MacroBodyOrigin, MacroCallId, MacroDefinitionId, MacroExpansionId, Token,
        TokenOrigin, Trace,
    },
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
) -> Trace {
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

mod definition_trace;
mod expansion_graph;
mod include_resolution;
mod macro_state;
mod missing_expansion;
mod missing_trace;
mod nested_expansion;
mod predefine_intrinsic;
mod trace;
