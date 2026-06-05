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
    let range = required_directive_range(source_order, kind, directive.range.as_ref())?;

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
    range: Option<&SourceBufferRange>,
) -> Result<SourceRange, SourcePreprocError> {
    range
        .and_then(source_range_from_trace)
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
