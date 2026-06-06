use std::collections::BTreeMap;

use smol_str::SmolStr;
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

    pub fn visible_macros_at(&self, position: SourcePosition) -> Vec<SourceMacroBinding<'_>> {
        self.tables
            .state_timeline
            .state_at_position(position)
            .map(|state| self.bindings_for_state(state))
            .unwrap_or_default()
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

    pub fn include_chain_for_source(
        &self,
        source: PreprocSourceId,
    ) -> Result<Vec<SourceIncludeChainEntry>, SourcePreprocError> {
        let mut chain = Vec::new();
        let mut current = source;
        let mut visited = BTreeMap::new();

        loop {
            if visited.insert(current, ()).is_some() {
                return Err(SourcePreprocError::IncludeCycle { source: current.raw() });
            }

            let Some(source) = self.index.sources.iter().find(|candidate| candidate.id == current)
            else {
                return Err(SourcePreprocError::MissingIncludedSource {
                    include_event_id: 0,
                    source: current.raw(),
                });
            };

            match source.origin {
                PreprocSourceOrigin::Root | PreprocSourceOrigin::Predefine => break,
                PreprocSourceOrigin::Detached => {
                    return Err(SourcePreprocError::MissingIncludeEdge { source: current.raw() });
                }
                PreprocSourceOrigin::Included { include_event_id } => {
                    let directive = self.event_record_by_event_id(include_event_id).ok_or(
                        SourcePreprocError::MissingIncludeEvent {
                            include_event_id: include_event_id.raw(),
                        },
                    )?;
                    if directive.kind != MacroEventKind::Include {
                        return Err(SourcePreprocError::IncludeEdgeNotInclude {
                            include_event_id: include_event_id.raw(),
                        });
                    }
                    chain.push(SourceIncludeChainEntry {
                        include_event_id,
                        include_range: directive.range,
                        included_source: current,
                    });
                    current = directive.range.source;
                }
            }
        }

        chain.reverse();
        Ok(chain)
    }

    fn event_record_by_event_id(
        &self,
        event_id: SourcePreprocEventId,
    ) -> Option<&SourcePreprocEventRecord> {
        self.index.event_records.iter().find(|directive| directive.event_id == event_id)
    }

    fn bindings_for_state(&self, state: &SourceMacroState) -> Vec<SourceMacroBinding<'_>> {
        state
            .definitions
            .iter()
            .filter_map(|(name, definition_id)| {
                let define_index = self.define_index_for_definition_id(*definition_id)?;
                self.binding_for_define_index(name.clone(), define_index)
            })
            .collect()
    }

    pub(super) fn binding_for_define_index(
        &self,
        name: SmolStr,
        define_index: usize,
    ) -> Option<SourceMacroBinding<'_>> {
        let define = self.index.defines.get(define_index)?;
        Some(SourceMacroBinding { name, event_id: define.event_id, define_index, define })
    }

    pub(super) fn binding_for_definition_id(
        &self,
        definition_id: SourceMacroDefinitionId,
    ) -> Option<SourceMacroBinding<'_>> {
        let definition = self.tables.macro_definitions.get(definition_id)?;
        let define_index = self.define_index_for_definition_id(definition_id)?;
        self.binding_for_define_index(definition.name.clone(), define_index)
    }

    fn define_index_for_definition_id(
        &self,
        definition_id: SourceMacroDefinitionId,
    ) -> Option<usize> {
        let definition = self.tables.macro_definitions.get(definition_id)?;
        self.index.defines.iter().position(|define| define.event_id == definition.event_id)
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
    use syntax::{
        PreprocessorTrace, PreprocessorTraceEvent, PreprocessorTraceEventId,
        PreprocessorTraceToken, SourceBufferId, SourceBufferOrigin, SourceBufferRange, SyntaxKind,
        SyntaxTree, SyntaxTreeBuffer, SyntaxTreeOptions,
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
            .map(|binding| binding.name)
            .collect()
    }

    fn visible_macro_binding<'a>(
        model: &'a SourcePreprocModel,
        source: PreprocSourceId,
        offset: TextSize,
        name: &str,
    ) -> Option<SourceMacroBinding<'a>> {
        model
            .visible_macros_at(SourcePosition { source, offset })
            .into_iter()
            .find(|binding| binding.name == name)
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

        let after_include = visible_macro_binding(
            &model,
            root_source,
            offset_after(root_text, "`include \"defs.vh\"\n"),
            "HEADER_WIDTH",
        )
        .unwrap();
        assert_eq!(after_include.define_index, 0);

        let binding = model
            .visible_macros_at(SourcePosition {
                source: root_source,
                offset: offset_after(root_text, "`include \"defs.vh\"\n"),
            })
            .into_iter()
            .find(|binding| binding.name == "HEADER_WIDTH")
            .unwrap();
        assert_eq!(binding.define.name_range.unwrap().source, header_source);
    }

    #[test]
    fn source_model_undef_removes_included_define() {
        let root_text = r#"`include "defs.vh"
`undef HEADER_WIDTH
logic [`HEADER_WIDTH-1:0] data;
"#;
        let header_text = "`define HEADER_WIDTH 8\n";
        let (model, root_source, header_source) = source_model(root_text, header_text);

        let after_include = visible_macro_binding(
            &model,
            root_source,
            offset_after(root_text, "`include \"defs.vh\"\n"),
            "HEADER_WIDTH",
        )
        .unwrap();
        assert_eq!(after_include.define_index, 0);
        assert_eq!(model.defines()[0].name_range.unwrap().source, header_source);

        assert!(
            visible_macro_binding(
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

        let after_override = visible_macro_binding(
            &model,
            root_source,
            offset_after(root_text, "`define HEADER_WIDTH 16\n"),
            "HEADER_WIDTH",
        )
        .unwrap();
        assert_eq!(after_override.define_index, 1);

        let binding = model
            .visible_macros_at(SourcePosition {
                source: root_source,
                offset: offset_after(root_text, "`define HEADER_WIDTH 16\n"),
            })
            .into_iter()
            .find(|binding| binding.name == "HEADER_WIDTH")
            .unwrap();
        assert_eq!(binding.define.body[0].value.as_str(), "16");
        assert_eq!(binding.define.name_range.unwrap().source, root_source);
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

        let resolution = model.definition_for_usage(usage_index).unwrap().unwrap();
        assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
        assert_eq!(resolution.definition.define.name_range.unwrap().source, header_source);
        assert_eq!(resolution.definition.define.body[0].value.as_str(), "8");
        assert_eq!(resolution.definition_provenance.event_id, resolution.definition.event_id);
        assert_eq!(resolution.definition_include_chain.len(), 1);
        assert_eq!(resolution.definition_include_chain[0].include_range.source, root_source);
        assert_eq!(resolution.definition_include_chain[0].included_source, header_source);
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
        assert!(model.macro_calls().is_empty());
        assert!(model.macro_expansions().is_empty());
        assert!(model.emitted_tokens().is_empty());
        assert!(model.token_provenance().is_empty());
        assert!(matches!(
            &model.capabilities().macro_calls,
            CapabilityStatus::Unavailable(SourcePreprocUnavailable::MacroCallAuthorityUnavailable)
        ));
        assert!(matches!(
            &model.capabilities().macro_expansions,
            CapabilityStatus::Unavailable(SourcePreprocUnavailable::ExpansionAuthorityUnavailable)
        ));
        assert!(matches!(
            &model.capabilities().emitted_tokens,
            CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::EmittedTokenAuthorityUnavailable
            )
        ));
        assert!(matches!(
            &model.capabilities().emitted_token_provenance,
            CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::TokenProvenanceAuthorityUnavailable
            )
        ));
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
        let binding = model.definition_for_conditional_token(conditional_index, 0).unwrap();

        assert_eq!(binding.name.as_str(), "HEADER_FLAG");
        assert_eq!(binding.define.name_range.unwrap().source, header_source);

        let references = model.resolved_macro_references().unwrap();
        assert!(references.iter().any(|reference| {
            matches!(
                reference.site,
                SourceMacroReferenceSite::ConditionalToken {
                    conditional_index: site_conditional_index,
                    token_index: 0,
                } if site_conditional_index == conditional_index
            ) && reference.name.as_str() == "HEADER_FLAG"
                && reference.range.source == root_source
                && reference.definition.define.name_range.unwrap().source == header_source
        }));
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
        let binding = model.definition_for_conditional_token(conditional_index, 0).unwrap();

        assert_eq!(binding.name.as_str(), "HEADER_FLAG");
        assert_eq!(binding.define.name_range.unwrap().source, header_source);

        let references = model.resolved_macro_references().unwrap();
        assert!(references.iter().any(|reference| {
            matches!(
                reference.site,
                SourceMacroReferenceSite::IncludeGuardIfNDef {
                    conditional_index: site_conditional_index,
                    token_index: 0,
                } if site_conditional_index == conditional_index
            ) && reference.name.as_str() == "HEADER_FLAG"
                && reference.range.source == header_source
                && reference.definition.define.name_range.unwrap().source == header_source
        }));
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
        let resolution = model.definition_for_usage(usage_index).unwrap().unwrap();

        assert_eq!(resolution.definition.define.name_range.unwrap().source, leaf_source);
        assert_eq!(resolution.definition_include_chain.len(), 2);
        assert_eq!(resolution.definition_include_chain[0].include_range.source, root_source);
        assert_eq!(resolution.definition_include_chain[0].included_source, header_source);
        assert_eq!(resolution.definition_include_chain[1].include_range.source, header_source);
        assert_eq!(resolution.definition_include_chain[1].included_source, leaf_source);
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
                    range: Some(SourceBufferRange { buffer_id: 1, range: 8..13 }),
                }),
                include_file_name: None,
                params: Vec::new(),
                body_tokens: Vec::new(),
                expr_tokens: Vec::new(),
                disabled_ranges: Vec::new(),
            }],
            include_edges: Vec::new(),
        };

        assert_eq!(
            SourcePreprocModel::from_trace(trace).unwrap_err(),
            SourcePreprocError::MissingEventRange { source_order: 0, kind: MacroEventKind::Define }
        );
    }
}
