use std::collections::BTreeMap;

use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    PreprocessorTrace, PreprocessorTraceDirective, PreprocessorTraceMacroParam,
    PreprocessorTraceToken, SourceBufferRange, SyntaxKind,
};
use utils::line_index::{TextRange, TextSize};

use crate::index::{MacroConditionalKind, MacroDirectiveKind, MacroIncludeTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreprocSourceId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub source: PreprocSourceId,
    pub offset: TextSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    pub source: PreprocSourceId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocSource {
    pub id: PreprocSourceId,
    pub path: SmolStr,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePreprocIndex {
    pub root_source: Option<PreprocSourceId>,
    pub sources: Vec<PreprocSource>,
    pub directives: Vec<SourceMacroDirective>,
    pub defines: Vec<SourceMacroDefine>,
    pub undefs: Vec<SourceMacroUndef>,
    pub includes: Vec<SourceMacroInclude>,
    pub conditionals: Vec<SourceMacroConditional>,
    pub usages: Vec<SourceMacroUsage>,
    pub inactive_ranges: Vec<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroDirective {
    pub kind: MacroDirectiveKind,
    pub range: SourceRange,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroDefine {
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub params: Option<Vec<SourceMacroParam>>,
    pub body: Vec<SourceMacroToken>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroParam {
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub default: Option<Vec<SourceMacroToken>>,
    pub range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroUndef {
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroInclude {
    pub target: MacroIncludeTarget,
    pub target_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroConditional {
    pub kind: MacroConditionalKind,
    pub expr: Vec<SourceMacroToken>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroUsage {
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroToken {
    pub raw: SmolStr,
    pub value: SmolStr,
    pub range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocModel {
    index: SourcePreprocIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceMacroEnvironment {
    definitions: BTreeMap<SmolStr, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroBinding<'a> {
    pub name: SmolStr,
    pub define_index: usize,
    pub define: &'a SourceMacroDefine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePreprocEntity {
    Define(usize),
    Undef(usize),
    Usage(usize),
    Include(usize),
    Conditional(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocProvenance {
    pub entity: SourcePreprocEntity,
    pub name: Option<SmolStr>,
    pub range: SourceRange,
    pub name_range: Option<SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePreprocEvent<'a> {
    Define { source_order: usize, index: usize, define: &'a SourceMacroDefine },
    Undef { source_order: usize, index: usize, undef: &'a SourceMacroUndef },
    Include { source_order: usize, index: usize, include: &'a SourceMacroInclude },
    Conditional { source_order: usize, index: usize, conditional: &'a SourceMacroConditional },
    Branch { source_order: usize, index: usize, conditional: &'a SourceMacroConditional },
    Usage { source_order: usize, index: usize, usage: &'a SourceMacroUsage },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocError {
    MissingRootSource,
    MissingDirectiveRange { source_order: usize, kind: MacroDirectiveKind },
}

impl PreprocSourceId {
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<u32> for PreprocSourceId {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl SourcePreprocIndex {
    pub fn from_trace(trace: PreprocessorTrace) -> Result<Self, SourcePreprocError> {
        let root_source = PreprocSourceId::from(trace.root_buffer_id);
        let mut index = Self {
            root_source: Some(root_source),
            sources: trace
                .source_buffers
                .into_iter()
                .map(|source| PreprocSource {
                    id: PreprocSourceId::from(source.buffer_id),
                    path: source.path.to_smolstr(),
                })
                .collect(),
            ..Self::default()
        };

        if !index.sources.iter().any(|source| source.id == root_source) {
            return Err(SourcePreprocError::MissingRootSource);
        }

        for (source_order, directive) in trace.directives.into_iter().enumerate() {
            collect_trace_directive(&mut index, source_order, directive)?;
        }

        Ok(index)
    }
}

impl SourcePreprocModel {
    pub fn new(index: SourcePreprocIndex) -> Self {
        Self { index }
    }

    pub fn from_trace(trace: PreprocessorTrace) -> Result<Self, SourcePreprocError> {
        SourcePreprocIndex::from_trace(trace).map(Self::new)
    }

    pub fn index(&self) -> &SourcePreprocIndex {
        &self.index
    }

    pub fn into_index(self) -> SourcePreprocIndex {
        self.index
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
        &self.index.inactive_ranges
    }

    pub fn events(&self) -> impl Iterator<Item = SourcePreprocEvent<'_>> + '_ {
        self.index.directives.iter().enumerate().filter_map(|(source_order, directive)| {
            self.event_from_directive(source_order, directive)
        })
    }

    pub fn macro_environment_at(&self, position: SourcePosition) -> SourceMacroEnvironment {
        let mut environment = SourceMacroEnvironment::default();
        let end_order = self.source_order_at_position(position);
        for directive in self.index.directives.iter().take(end_order) {
            self.apply_macro_state(directive, &mut environment);
        }
        environment
    }

    pub fn visible_macros_at(&self, position: SourcePosition) -> Vec<SourceMacroBinding<'_>> {
        let environment = self.macro_environment_at(position);
        self.bindings_for_environment(&environment)
    }

    pub fn definition_for_usage(&self, usage_index: usize) -> Option<SourceMacroBinding<'_>> {
        let usage = self.index.usages.get(usage_index)?;
        let name = usage.name.as_ref()?;
        let environment = self.macro_environment_before(SourcePreprocEntity::Usage(usage_index))?;
        let define_index = environment.define_index(name.as_str())?;
        let define = self.index.defines.get(define_index)?;
        Some(SourceMacroBinding { name: name.clone(), define_index, define })
    }

    pub fn provenance(&self, entity: SourcePreprocEntity) -> Option<SourcePreprocProvenance> {
        let (name, range, name_range) = match entity {
            SourcePreprocEntity::Define(index) => {
                let define = self.index.defines.get(index)?;
                (define.name.clone(), define.range, define.name_range)
            }
            SourcePreprocEntity::Undef(index) => {
                let undef = self.index.undefs.get(index)?;
                (undef.name.clone(), undef.range, undef.name_range)
            }
            SourcePreprocEntity::Usage(index) => {
                let usage = self.index.usages.get(index)?;
                (usage.name.clone(), usage.range, usage.name_range)
            }
            SourcePreprocEntity::Include(index) => {
                let include = self.index.includes.get(index)?;
                (None, include.range, include.target_range)
            }
            SourcePreprocEntity::Conditional(index) => {
                let conditional = self.index.conditionals.get(index)?;
                (None, conditional.range, None)
            }
        };
        Some(SourcePreprocProvenance { entity, name, range, name_range })
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

    fn source_order_at_position(&self, position: SourcePosition) -> usize {
        self.index
            .directives
            .iter()
            .enumerate()
            .find(|(_, directive)| {
                directive.range.source == position.source
                    && directive.range.range.end() > position.offset
            })
            .map(|(source_order, _)| source_order)
            .unwrap_or(self.index.directives.len())
    }

    fn macro_environment_before(
        &self,
        entity: SourcePreprocEntity,
    ) -> Option<SourceMacroEnvironment> {
        let mut environment = SourceMacroEnvironment::default();
        for directive in &self.index.directives {
            if source_directive_matches_entity(directive, entity) {
                return Some(environment);
            }
            self.apply_macro_state(directive, &mut environment);
        }
        None
    }

    fn bindings_for_environment(
        &self,
        environment: &SourceMacroEnvironment,
    ) -> Vec<SourceMacroBinding<'_>> {
        environment
            .definitions
            .iter()
            .filter_map(|(name, define_index)| {
                let define = self.index.defines.get(*define_index)?;
                Some(SourceMacroBinding { name: name.clone(), define_index: *define_index, define })
            })
            .collect()
    }

    fn apply_macro_state(
        &self,
        directive: &SourceMacroDirective,
        environment: &mut SourceMacroEnvironment,
    ) {
        match directive.kind {
            MacroDirectiveKind::Define => {
                if let Some(define) = self.index.defines.get(directive.index)
                    && let Some(name) = define.name.as_ref()
                {
                    environment.definitions.insert(name.clone(), directive.index);
                }
            }
            MacroDirectiveKind::Undef => {
                if let Some(undef) = self.index.undefs.get(directive.index)
                    && let Some(name) = undef.name.as_ref()
                {
                    environment.definitions.remove(name.as_str());
                }
            }
            MacroDirectiveKind::Include
            | MacroDirectiveKind::Conditional
            | MacroDirectiveKind::Branch
            | MacroDirectiveKind::Usage => {}
        }
    }

    fn event_from_directive(
        &self,
        source_order: usize,
        directive: &SourceMacroDirective,
    ) -> Option<SourcePreprocEvent<'_>> {
        match directive.kind {
            MacroDirectiveKind::Define => {
                let define = self.index.defines.get(directive.index)?;
                Some(SourcePreprocEvent::Define { source_order, index: directive.index, define })
            }
            MacroDirectiveKind::Undef => {
                let undef = self.index.undefs.get(directive.index)?;
                Some(SourcePreprocEvent::Undef { source_order, index: directive.index, undef })
            }
            MacroDirectiveKind::Include => {
                let include = self.index.includes.get(directive.index)?;
                Some(SourcePreprocEvent::Include { source_order, index: directive.index, include })
            }
            MacroDirectiveKind::Conditional => {
                let conditional = self.index.conditionals.get(directive.index)?;
                Some(SourcePreprocEvent::Conditional {
                    source_order,
                    index: directive.index,
                    conditional,
                })
            }
            MacroDirectiveKind::Branch => {
                let conditional = self.index.conditionals.get(directive.index)?;
                Some(SourcePreprocEvent::Branch {
                    source_order,
                    index: directive.index,
                    conditional,
                })
            }
            MacroDirectiveKind::Usage => {
                let usage = self.index.usages.get(directive.index)?;
                Some(SourcePreprocEvent::Usage { source_order, index: directive.index, usage })
            }
        }
    }
}

fn collect_trace_directive(
    index: &mut SourcePreprocIndex,
    source_order: usize,
    directive: PreprocessorTraceDirective,
) -> Result<(), SourcePreprocError> {
    index.inactive_ranges.extend(
        directive
            .disabled_ranges
            .iter()
            .filter_map(source_range_from_trace)
            .filter(|range| !range.range.is_empty()),
    );

    let Some(kind) = directive_kind(directive.kind) else {
        return Ok(());
    };
    let range = required_directive_range(source_order, kind, &directive)?;

    match kind {
        MacroDirectiveKind::Define => {
            let directive_index = index.defines.len();
            let define = collect_trace_define(directive, range);
            index.defines.push(define);
            push_source_directive(index, kind, directive_index, range);
        }
        MacroDirectiveKind::Undef => {
            let directive_index = index.undefs.len();
            index.undefs.push(SourceMacroUndef {
                name: directive.name.as_ref().map(trace_token_value),
                name_range: directive.name.as_ref().and_then(trace_token_range),
                range,
            });
            push_source_directive(index, kind, directive_index, range);
        }
        MacroDirectiveKind::Include => {
            let directive_index = index.includes.len();
            let target = directive
                .include_file_name
                .as_ref()
                .map(|token| include_target_from_raw(token.raw_text.to_smolstr()))
                .unwrap_or_else(|| MacroIncludeTarget::Token { raw: SmolStr::new("") });
            index.includes.push(SourceMacroInclude {
                target,
                target_range: directive.include_file_name.as_ref().and_then(trace_token_range),
                range,
            });
            push_source_directive(index, kind, directive_index, range);
        }
        MacroDirectiveKind::Conditional | MacroDirectiveKind::Branch => {
            let directive_index = index.conditionals.len();
            index.conditionals.push(SourceMacroConditional {
                kind: trace_conditional_kind(directive.kind),
                expr: directive.expr_tokens.into_iter().map(macro_token_from_trace).collect(),
                range,
            });
            push_source_directive(index, kind, directive_index, range);
        }
        MacroDirectiveKind::Usage => {
            let directive_index = index.usages.len();
            index.usages.push(SourceMacroUsage {
                name: directive.name.as_ref().map(|token| macro_name(token.value_text.as_str())),
                name_range: directive.name.as_ref().and_then(trace_token_range),
                range,
            });
            push_source_directive(index, kind, directive_index, range);
        }
    }

    Ok(())
}

fn collect_trace_define(
    directive: PreprocessorTraceDirective,
    range: SourceRange,
) -> SourceMacroDefine {
    SourceMacroDefine {
        name: directive.name.as_ref().map(trace_token_value),
        name_range: directive.name.as_ref().and_then(trace_token_range),
        params: (!directive.params.is_empty())
            .then(|| directive.params.into_iter().map(macro_param_from_trace).collect()),
        body: directive.body_tokens.into_iter().map(macro_token_from_trace).collect(),
        range,
    }
}

fn macro_param_from_trace(param: PreprocessorTraceMacroParam) -> SourceMacroParam {
    SourceMacroParam {
        name: param.name.as_ref().map(trace_token_value),
        name_range: param.name.as_ref().and_then(trace_token_range),
        default: param
            .default_tokens
            .map(|tokens| tokens.into_iter().map(macro_token_from_trace).collect()),
        range: param.range.as_ref().and_then(source_range_from_trace),
    }
}

fn macro_token_from_trace(token: PreprocessorTraceToken) -> SourceMacroToken {
    SourceMacroToken {
        raw: token.raw_text.to_smolstr(),
        value: token.value_text.to_smolstr(),
        range: token.range.as_ref().and_then(source_range_from_trace),
    }
}

fn trace_token_value(token: &PreprocessorTraceToken) -> SmolStr {
    token.value_text.to_smolstr()
}

fn trace_token_range(token: &PreprocessorTraceToken) -> Option<SourceRange> {
    token.range.as_ref().and_then(source_range_from_trace)
}

fn required_directive_range(
    source_order: usize,
    kind: MacroDirectiveKind,
    directive: &PreprocessorTraceDirective,
) -> Result<SourceRange, SourcePreprocError> {
    directive
        .range
        .as_ref()
        .and_then(source_range_from_trace)
        .or_else(|| directive.directive.as_ref().and_then(trace_token_range))
        .or_else(|| match kind {
            MacroDirectiveKind::Define | MacroDirectiveKind::Undef | MacroDirectiveKind::Usage => {
                directive.name.as_ref().and_then(trace_token_range)
            }
            MacroDirectiveKind::Include => {
                directive.include_file_name.as_ref().and_then(trace_token_range)
            }
            MacroDirectiveKind::Conditional => {
                directive.expr_tokens.first().and_then(trace_token_range)
            }
            MacroDirectiveKind::Branch => None,
        })
        .ok_or(SourcePreprocError::MissingDirectiveRange { source_order, kind })
}

fn source_range_from_trace(range: &SourceBufferRange) -> Option<SourceRange> {
    Some(SourceRange {
        source: PreprocSourceId::from(range.buffer_id),
        range: TextRange::new(
            TextSize::from(u32::try_from(range.range.start).ok()?),
            TextSize::from(u32::try_from(range.range.end).ok()?),
        ),
    })
}

fn directive_kind(kind: SyntaxKind) -> Option<MacroDirectiveKind> {
    match kind {
        SyntaxKind::DEFINE_DIRECTIVE => Some(MacroDirectiveKind::Define),
        SyntaxKind::UNDEF_DIRECTIVE => Some(MacroDirectiveKind::Undef),
        SyntaxKind::INCLUDE_DIRECTIVE => Some(MacroDirectiveKind::Include),
        SyntaxKind::IF_DEF_DIRECTIVE
        | SyntaxKind::IF_N_DEF_DIRECTIVE
        | SyntaxKind::ELS_IF_DIRECTIVE => Some(MacroDirectiveKind::Conditional),
        SyntaxKind::ELSE_DIRECTIVE | SyntaxKind::END_IF_DIRECTIVE => {
            Some(MacroDirectiveKind::Branch)
        }
        SyntaxKind::MACRO_USAGE => Some(MacroDirectiveKind::Usage),
        _ => None,
    }
}

fn trace_conditional_kind(kind: SyntaxKind) -> MacroConditionalKind {
    match kind {
        SyntaxKind::IF_DEF_DIRECTIVE => MacroConditionalKind::IfDef,
        SyntaxKind::IF_N_DEF_DIRECTIVE => MacroConditionalKind::IfNDef,
        SyntaxKind::ELS_IF_DIRECTIVE => MacroConditionalKind::ElsIf,
        SyntaxKind::ELSE_DIRECTIVE => MacroConditionalKind::Else,
        SyntaxKind::END_IF_DIRECTIVE => MacroConditionalKind::EndIf,
        _ => unreachable!(),
    }
}

fn push_source_directive(
    index: &mut SourcePreprocIndex,
    kind: MacroDirectiveKind,
    directive_index: usize,
    range: SourceRange,
) {
    index.directives.push(SourceMacroDirective { kind, range, index: directive_index });
}

fn include_target_from_raw(raw: SmolStr) -> MacroIncludeTarget {
    if let Some(path) = strip_include_delimiters(&raw) {
        MacroIncludeTarget::Literal { path: path.to_smolstr(), raw }
    } else {
        MacroIncludeTarget::Token { raw }
    }
}

fn strip_include_delimiters(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let (first, last) = (*bytes.first()?, *bytes.last()?);
    match (first, last) {
        (b'"', b'"') | (b'<', b'>') if raw.len() >= 2 => Some(&raw[1..raw.len() - 1]),
        _ => None,
    }
}

fn macro_name(name: &str) -> SmolStr {
    name.strip_prefix('`').unwrap_or(name).to_smolstr()
}

impl SourceMacroEnvironment {
    pub fn define_index(&self, name: &str) -> Option<usize> {
        self.definitions.get(name).copied()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &SmolStr> {
        self.definitions.keys()
    }

    pub fn definitions(&self) -> &BTreeMap<SmolStr, usize> {
        &self.definitions
    }
}

impl SourcePreprocEvent<'_> {
    pub fn source_order(&self) -> usize {
        match self {
            SourcePreprocEvent::Define { source_order, .. }
            | SourcePreprocEvent::Undef { source_order, .. }
            | SourcePreprocEvent::Include { source_order, .. }
            | SourcePreprocEvent::Conditional { source_order, .. }
            | SourcePreprocEvent::Branch { source_order, .. }
            | SourcePreprocEvent::Usage { source_order, .. } => *source_order,
        }
    }

    pub fn kind(&self) -> MacroDirectiveKind {
        match self {
            SourcePreprocEvent::Define { .. } => MacroDirectiveKind::Define,
            SourcePreprocEvent::Undef { .. } => MacroDirectiveKind::Undef,
            SourcePreprocEvent::Include { .. } => MacroDirectiveKind::Include,
            SourcePreprocEvent::Conditional { .. } => MacroDirectiveKind::Conditional,
            SourcePreprocEvent::Branch { .. } => MacroDirectiveKind::Branch,
            SourcePreprocEvent::Usage { .. } => MacroDirectiveKind::Usage,
        }
    }

    pub fn entity(&self) -> SourcePreprocEntity {
        match self {
            SourcePreprocEvent::Define { index, .. } => SourcePreprocEntity::Define(*index),
            SourcePreprocEvent::Undef { index, .. } => SourcePreprocEntity::Undef(*index),
            SourcePreprocEvent::Include { index, .. } => SourcePreprocEntity::Include(*index),
            SourcePreprocEvent::Conditional { index, .. }
            | SourcePreprocEvent::Branch { index, .. } => SourcePreprocEntity::Conditional(*index),
            SourcePreprocEvent::Usage { index, .. } => SourcePreprocEntity::Usage(*index),
        }
    }

    pub fn range(&self) -> SourceRange {
        match self {
            SourcePreprocEvent::Define { define, .. } => define.range,
            SourcePreprocEvent::Undef { undef, .. } => undef.range,
            SourcePreprocEvent::Include { include, .. } => include.range,
            SourcePreprocEvent::Conditional { conditional, .. }
            | SourcePreprocEvent::Branch { conditional, .. } => conditional.range,
            SourcePreprocEvent::Usage { usage, .. } => usage.range,
        }
    }
}

fn source_directive_matches_entity(
    directive: &SourceMacroDirective,
    entity: SourcePreprocEntity,
) -> bool {
    match (directive.kind, entity) {
        (MacroDirectiveKind::Define, SourcePreprocEntity::Define(index))
        | (MacroDirectiveKind::Undef, SourcePreprocEntity::Undef(index))
        | (MacroDirectiveKind::Usage, SourcePreprocEntity::Usage(index))
        | (MacroDirectiveKind::Include, SourcePreprocEntity::Include(index)) => {
            directive.index == index
        }
        (
            MacroDirectiveKind::Conditional | MacroDirectiveKind::Branch,
            SourcePreprocEntity::Conditional(index),
        ) => directive.index == index,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use syntax::{PreprocessorTrace, SyntaxTree, SyntaxTreeBuffer, SyntaxTreeOptions};

    use super::*;

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
            .directives
            .iter()
            .filter_map(|directive| directive.range.as_ref())
            .map(|range| PreprocSourceId::from(range.buffer_id))
            .find(|source| *source != root_source)
            .expect("included source directive should be traced")
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

    #[test]
    fn source_model_applies_include_define_after_include_point_only() {
        let root_text = r#"`include "defs.vh"
logic [`HEADER_WIDTH-1:0] data;
"#;
        let header_text = "`define HEADER_WIDTH 8\n";
        let (model, root_source, header_source) = source_model(root_text, header_text);

        let before_include = model.macro_environment_at(SourcePosition {
            source: root_source,
            offset: offset_before(root_text, "`include"),
        });
        assert!(!before_include.contains("HEADER_WIDTH"));

        let after_include = model.macro_environment_at(SourcePosition {
            source: root_source,
            offset: offset_after(root_text, "`include \"defs.vh\"\n"),
        });
        assert_eq!(after_include.define_index("HEADER_WIDTH"), Some(0));

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

        let after_include = model.macro_environment_at(SourcePosition {
            source: root_source,
            offset: offset_after(root_text, "`include \"defs.vh\"\n"),
        });
        assert_eq!(after_include.define_index("HEADER_WIDTH"), Some(0));
        assert_eq!(model.defines()[0].name_range.unwrap().source, header_source);

        let after_undef = model.macro_environment_at(SourcePosition {
            source: root_source,
            offset: offset_after(root_text, "`undef HEADER_WIDTH\n"),
        });
        assert!(!after_undef.contains("HEADER_WIDTH"));
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

        let after_override = model.macro_environment_at(SourcePosition {
            source: root_source,
            offset: offset_after(root_text, "`define HEADER_WIDTH 16\n"),
        });
        assert_eq!(after_override.define_index("HEADER_WIDTH"), Some(1));

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

        let binding = model.definition_for_usage(usage_index).unwrap();
        assert_eq!(binding.name.as_str(), "HEADER_WIDTH");
        assert_eq!(binding.define.name_range.unwrap().source, header_source);
        assert_eq!(binding.define.body[0].value.as_str(), "8");
    }
}
