use syntax::PreprocessorTrace;

use super::{provenance::*, types::*};

impl SourcePreprocModel {
    pub fn new(index: SourcePreprocIndex) -> Self {
        let tables = SourcePreprocTables::from_index(&index);
        Self { index, tables }
    }

    pub fn from_trace(trace: PreprocessorTrace) -> Result<Self, SourcePreprocError> {
        let index = SourcePreprocIndex::from_trace(trace)?;
        Ok(Self::new(index))
    }

    pub fn index(&self) -> &SourcePreprocIndex {
        &self.index
    }

    pub fn into_index(self) -> SourcePreprocIndex {
        self.index
    }

    pub fn provenance_tables(&self) -> &SourcePreprocTables {
        &self.tables
    }

    pub fn macro_definitions(&self) -> &SourceMacroDefinitionTable {
        &self.tables.macro_definitions
    }

    pub fn macro_references(&self) -> &SourceMacroReferenceTable {
        &self.tables.macro_references
    }

    pub fn macro_calls(&self) -> &SourceMacroCallTable {
        &self.tables.macro_calls
    }

    pub fn macro_expansions(&self) -> &SourceMacroExpansionTable {
        &self.tables.macro_expansions
    }

    pub fn emitted_tokens(&self) -> &SourceEmittedTokenTable {
        &self.tables.emitted_tokens
    }

    pub fn token_provenance(&self) -> &SourceTokenProvenanceTable {
        &self.tables.token_provenance
    }

    pub fn include_graph(&self) -> &SourceIncludeGraph {
        &self.tables.include_graph
    }

    pub fn state_timeline(&self) -> &SourceMacroStateTimeline {
        &self.tables.state_timeline
    }

    pub fn capabilities(&self) -> &SourcePreprocCapabilities {
        &self.tables.capabilities
    }

    pub fn root_source(&self) -> Option<PreprocSourceId> {
        self.index.root_source
    }

    pub fn sources(&self) -> &[PreprocSource] {
        &self.index.sources
    }

    pub fn defines(&self) -> &[SourceMacroDefine] {
        &self.index.defines
    }

    pub fn undefs(&self) -> &[SourceMacroUndef] {
        &self.index.undefs
    }

    pub fn usages(&self) -> &[SourceMacroUsage] {
        &self.index.usages
    }

    pub fn includes(&self) -> &[SourceMacroInclude] {
        &self.index.includes
    }

    pub fn conditionals(&self) -> &[SourceMacroConditional] {
        &self.index.conditionals
    }

    pub fn inactive_ranges(&self) -> &[SourceRange] {
        &self.tables.inactive_ranges
    }

    pub fn events(&self) -> impl Iterator<Item = SourcePreprocEvent<'_>> + '_ {
        self.index
            .event_records
            .iter()
            .enumerate()
            .filter_map(|(source_order, directive)| self.event_from_record(source_order, directive))
    }

    pub fn visible_macros_at(&self, position: SourcePosition) -> Vec<&SourceMacroDefinition> {
        self.tables
            .state_timeline
            .state_at_position(position)
            .map(|state| self.definitions_for_state(state))
            .unwrap_or_default()
    }

    pub fn immediate_macro_expansion(&self, call: SourceMacroCallId) -> SourceMacroExpansionQuery {
        let Some(call_fact) = self.tables.macro_calls.get(call) else {
            return SourceMacroExpansionQuery::Unavailable(
                SourcePreprocUnavailable::MissingMacroCall { call },
            );
        };
        match (call_fact.expansion, &call_fact.status) {
            (Some(expansion), SourceMacroCallStatus::ExpansionAvailable)
                if self.tables.macro_expansions.get(expansion).is_some() =>
            {
                SourceMacroExpansionQuery::Available(expansion)
            }
            (Some(expansion), SourceMacroCallStatus::ExpansionAvailable) => {
                SourceMacroExpansionQuery::Unavailable(
                    SourcePreprocUnavailable::MissingMacroExpansion {
                        call: self
                            .tables
                            .macro_expansions
                            .get(expansion)
                            .map(|expansion| expansion.call)
                            .unwrap_or(call),
                    },
                )
            }
            (_, SourceMacroCallStatus::ExpansionUnavailable(reason)) => {
                SourceMacroExpansionQuery::Unavailable(reason.clone())
            }
            (None, SourceMacroCallStatus::ExpansionAvailable) => {
                SourceMacroExpansionQuery::Unavailable(
                    SourcePreprocUnavailable::MissingMacroExpansion { call },
                )
            }
        }
    }

    pub fn recursive_macro_expansion(
        &self,
        call: SourceMacroCallId,
    ) -> SourceRecursiveMacroExpansion {
        let mut result = SourceRecursiveMacroExpansion {
            root_call: call,
            expansions: Vec::new(),
            unavailable: Vec::new(),
        };
        self.collect_recursive_macro_expansion(call, &mut result, &mut Vec::new());
        result
    }

    pub fn provenance(&self, entity: SourcePreprocEntity) -> Option<SourcePreprocProvenance> {
        let (event_id, name, range, name_range) = match entity {
            SourcePreprocEntity::Define(index) => {
                let define = self.index.defines.get(index)?;
                (define.event_id, define.name.clone(), define.range, define.name_range)
            }
            SourcePreprocEntity::Undef(index) => {
                let undef = self.index.undefs.get(index)?;
                (undef.event_id, undef.name.clone(), undef.range, undef.name_range)
            }
            SourcePreprocEntity::Usage(index) => {
                let usage = self.index.usages.get(index)?;
                (usage.event_id, usage.name.clone(), usage.range, usage.name_range)
            }
            SourcePreprocEntity::Include(index) => {
                let include = self.index.includes.get(index)?;
                (include.event_id, None, include.range, include.target_range)
            }
            SourcePreprocEntity::Conditional(index) => {
                let conditional = self.index.conditionals.get(index)?;
                (conditional.event_id, None, conditional.range, None)
            }
        };
        Some(SourcePreprocProvenance { event_id, entity, name, range, name_range })
    }

    pub fn source_range(&self, entity: SourcePreprocEntity) -> Option<SourceRange> {
        self.provenance(entity).map(|provenance| provenance.range)
    }

    pub fn define(&self, index: usize) -> Option<&SourceMacroDefine> {
        self.index.defines.get(index)
    }

    pub fn undef(&self, index: usize) -> Option<&SourceMacroUndef> {
        self.index.undefs.get(index)
    }

    pub fn usage(&self, index: usize) -> Option<&SourceMacroUsage> {
        self.index.usages.get(index)
    }

    pub fn include(&self, index: usize) -> Option<&SourceMacroInclude> {
        self.index.includes.get(index)
    }

    pub fn conditional(&self, index: usize) -> Option<&SourceMacroConditional> {
        self.index.conditionals.get(index)
    }

    fn definitions_for_state(&self, state: &SourceMacroState) -> Vec<&SourceMacroDefinition> {
        state
            .definitions
            .values()
            .filter_map(|definition_id| self.tables.macro_definitions.get(*definition_id))
            .collect()
    }

    fn collect_recursive_macro_expansion(
        &self,
        call: SourceMacroCallId,
        result: &mut SourceRecursiveMacroExpansion,
        visiting: &mut Vec<SourceMacroCallId>,
    ) {
        if visiting.contains(&call) {
            result.unavailable.push(SourceMacroExpansionUnavailable {
                call,
                reason: SourcePreprocUnavailable::MissingMacroExpansion { call },
            });
            return;
        }

        match self.immediate_macro_expansion(call) {
            SourceMacroExpansionQuery::Available(expansion_id) => {
                if result.expansions.contains(&expansion_id) {
                    return;
                }
                result.expansions.push(expansion_id);
                let Some(expansion) = self.tables.macro_expansions.get(expansion_id) else {
                    result.unavailable.push(SourceMacroExpansionUnavailable {
                        call,
                        reason: SourcePreprocUnavailable::MissingMacroExpansion { call },
                    });
                    return;
                };
                visiting.push(call);
                for child in &expansion.child_calls {
                    self.collect_recursive_macro_expansion(*child, result, visiting);
                }
                visiting.pop();
            }
            SourceMacroExpansionQuery::Unavailable(reason) => {
                result.unavailable.push(SourceMacroExpansionUnavailable { call, reason });
            }
        }
    }

    fn event_from_record(
        &self,
        source_order: usize,
        directive: &SourcePreprocEventRecord,
    ) -> Option<SourcePreprocEvent<'_>> {
        match directive.kind {
            MacroEventKind::Define => {
                let define = self.index.defines.get(directive.index)?;
                Some(SourcePreprocEvent::Define {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    define,
                })
            }
            MacroEventKind::Undef => {
                let undef = self.index.undefs.get(directive.index)?;
                Some(SourcePreprocEvent::Undef {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    undef,
                })
            }
            MacroEventKind::Include => {
                let include = self.index.includes.get(directive.index)?;
                Some(SourcePreprocEvent::Include {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    include,
                })
            }
            MacroEventKind::Conditional => {
                let conditional = self.index.conditionals.get(directive.index)?;
                Some(SourcePreprocEvent::Conditional {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    conditional,
                })
            }
            MacroEventKind::Branch => {
                let conditional = self.index.conditionals.get(directive.index)?;
                Some(SourcePreprocEvent::Branch {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    conditional,
                })
            }
            MacroEventKind::Usage => {
                let usage = self.index.usages.get(directive.index)?;
                Some(SourcePreprocEvent::Usage {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    usage,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use syntax::{
        PreprocessorTrace, PreprocessorTraceEvent, PreprocessorTraceEventId,
        PreprocessorTraceToken, SourceBufferId, SourceBufferOrigin, SourceBufferRange, SyntaxKind,
        SyntaxTree, SyntaxTreeBuffer, SyntaxTreeOptions, TokenKind,
    };
    use utils::line_index::{TextRange, TextSize};

    use super::{super::SourceMacroReferenceSite, *};

    const ROOT_PATH: &str = "sample/rtl/top.sv";
    const HEADER_PATH: &str = "sample/include/defs.vh";
    const INCLUDE_DIR: &str = "sample/include";

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
        let trace = SyntaxTree::preprocessor_trace(root_text, "source", ROOT_PATH, &options)
            .expect("trace should include root source");
        let root_source = PreprocSourceId::from(trace.root_buffer_id);
        let header_source = first_non_root_source(&trace, root_source);
        let model = SourcePreprocModel::from_trace(trace).unwrap();
        (model, root_source, header_source)
    }

    fn first_non_root_source(
        trace: &PreprocessorTrace,
        root_source: PreprocSourceId,
    ) -> PreprocSourceId {
        trace
            .events
            .iter()
            .filter_map(|directive| directive.range.as_ref())
            .map(|range| PreprocSourceId::from(range.buffer_id))
            .find(|source| *source != root_source)
            .expect("included source directive should be traced")
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
        let trace = SyntaxTree::preprocessor_trace(root_text, "source", ROOT_PATH, &options)
            .expect("trace should include root source");
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

    fn reference_for_usage(
        model: &SourcePreprocModel,
        usage_index: usize,
    ) -> &SourceMacroReference {
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
        let trace = SyntaxTree::preprocessor_trace(
            root_text,
            "source",
            ROOT_PATH,
            &SyntaxTreeOptions::default(),
        )
        .expect("trace should include root source");
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
        let SourceMacroResolution::Resolved {
            definition: resolved_definition,
            reason,
            include_chain,
        } = &reference.resolution
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
        assert_eq!(*resolved_definition, expansion.definition);
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
        let SourceTokenProvenance::MacroArgument { call, argument_index, argument_token_range } =
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
    fn source_model_builds_nested_macro_expansion_provenance_chain() {
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
                    && matches!(
                        reference.site,
                        SourceMacroReferenceSite::ExpansionToken { emitted_token: _ }
                    )
            })
            .expect("nested macro invocation should create an expansion-token call");
        let leaf_reference = model.macro_references().get(leaf_call.reference).unwrap();
        assert_eq!(text_at_range(root_text, leaf_reference.name_range.range), "`LEAF");

        let SourceMacroExpansionQuery::Available(wrap_expansion_id) =
            model.immediate_macro_expansion(wrap_call.id)
        else {
            panic!("outer macro should have expansion range from nested emitted tokens");
        };
        let wrap_expansion = model.macro_expansions().get(wrap_expansion_id).unwrap();
        assert_eq!(wrap_expansion.child_calls, vec![leaf_call.id]);

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
        let SourceTokenProvenance::MacroBody { call, .. } =
            model.token_provenance().get(emitted.provenance).unwrap()
        else {
            panic!("nested emitted token should keep macro body provenance");
        };
        assert_eq!(*call, leaf_call.id);
        assert_eq!(wrap_expansion.emitted_token_range.start, emitted.id);

        let recursive = model.recursive_macro_expansion(wrap_call.id);
        assert_eq!(recursive.expansions, vec![wrap_expansion_id, leaf_expansion_id]);
        assert!(recursive.unavailable.is_empty());
    }

    #[test]
    fn source_model_marks_unsupported_macro_ops_unavailable_without_dropping_tokens() {
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
        assert!(matches!(
            model.token_provenance().get(pasted.provenance).unwrap(),
            SourceTokenProvenance::Unavailable(
                SourcePreprocUnavailable::UnsupportedEmittedTokenProvenance
            )
        ));

        let stringified = model
            .emitted_tokens()
            .iter()
            .find(|token| token.text.as_str() == "\"foo\"")
            .expect("stringification result should not be dropped");
        assert!(matches!(
            model.token_provenance().get(stringified.provenance).unwrap(),
            SourceTokenProvenance::Unavailable(
                SourcePreprocUnavailable::UnsupportedEmittedTokenProvenance
            )
        ));
        assert_eq!(model.capabilities().emitted_tokens, CapabilityStatus::Complete);
        assert_eq!(model.capabilities().emitted_token_provenance, CapabilityStatus::Partial);
        assert_eq!(model.capabilities().macro_expansions, CapabilityStatus::Partial);
        for call in model.macro_calls().iter() {
            assert!(matches!(
                model.immediate_macro_expansion(call.id),
                SourceMacroExpansionQuery::Unavailable(_)
            ));
        }
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
                buffer_id: 1,
                origin: SourceBufferOrigin::Source,
            }],
            events: vec![
                PreprocessorTraceEvent {
                    event_id: PreprocessorTraceEventId(0),
                    kind: SyntaxKind::DEFINE_DIRECTIVE,
                    range: Some(SourceBufferRange {
                        buffer_id: 1,
                        range: define_start..define_end,
                    }),
                    directive: None,
                    name: Some(PreprocessorTraceToken {
                        raw_text: "A".to_owned(),
                        value_text: "A".to_owned(),
                        token_kind: TokenKind::IDENTIFIER,
                        range: Some(SourceBufferRange { buffer_id: 1, range: 8..9 }),
                    }),
                    include_file_name: None,
                    params: Vec::new(),
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
                SourcePreprocUnavailable::EmittedTokenAuthorityUnavailable
            )
        ));
        assert!(matches!(
            &model.capabilities().macro_expansions,
            CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::EmittedTokenAuthorityUnavailable
            )
        ));
    }

    #[test]
    fn source_model_maps_predefine_and_builtin_emitted_token_provenance() {
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

        let builtin = model
            .emitted_tokens()
            .iter()
            .find(|token| {
                matches!(
                    model.token_provenance().get(token.provenance),
                    Some(SourceTokenProvenance::Builtin { name }) if name == "__LINE__"
                )
            })
            .expect("builtin macro token should be emitted");
        assert!(!builtin.text.is_empty());
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
        let SourceMacroResolution::Resolved { definition, reason, .. } = reference.resolution
        else {
            panic!("conditional token reference should resolve to visible definition");
        };
        assert_eq!(reason, SourceMacroResolutionReason::VisibleDefinition);
        assert_eq!(
            model.macro_definitions().get(definition).unwrap().name_range.source,
            header_source
        );
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
        let trace = SyntaxTree::preprocessor_trace(root_text, "source", ROOT_PATH, &options)
            .expect("trace should include root source");
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
        let SourceMacroResolution::Resolved { definition, include_chain, .. } =
            &reference.resolution
        else {
            panic!("usage reference should resolve to nested included definition");
        };

        assert_eq!(
            model.macro_definitions().get(*definition).unwrap().name_range.source,
            leaf_source
        );
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
                buffer_id: 1,
                origin: SourceBufferOrigin::Source,
            }],
            events: vec![PreprocessorTraceEvent {
                event_id: PreprocessorTraceEventId(0),
                kind: SyntaxKind::DEFINE_DIRECTIVE,
                range: None,
                directive: None,
                name: Some(PreprocessorTraceToken {
                    raw_text: "WIDTH".to_owned(),
                    value_text: "WIDTH".to_owned(),
                    token_kind: TokenKind::IDENTIFIER,
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..13 }),
                }),
                include_file_name: None,
                params: Vec::new(),
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
}
