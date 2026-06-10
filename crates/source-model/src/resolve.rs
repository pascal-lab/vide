use crate::{
    FileRange,
    ids::{
        EntityId, ExpansionTokenId, HirReferenceId, HirSymbolId, IncludeDirectiveId, MacroCallId,
        MacroDefinitionId, MacroParamDefinitionId, MacroParamReferenceId, MacroReferenceId, SpanId,
        SyntaxTokenEntityId,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourcePurpose {
    Hover,
    GotoDefinition,
    FindReferences,
    Rename,
    Diagnostic,
    SemanticToken,
    Completion,
    CodeAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceTargetResolution {
    Resolved(ResolvedSourceTarget),
    Ambiguous(Vec<ResolvedSourceTarget>),
    Blocked(SourceBlock),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolvedSourceTarget {
    pub entity: EntityId,
    pub target: SourceTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceTarget {
    MacroDefinition(MacroDefinitionId),
    MacroReference(MacroReferenceId),
    MacroCall(MacroCallId),
    MacroParamDefinition(MacroParamDefinitionId),
    MacroParamReference(MacroParamReferenceId),
    Include(IncludeDirectiveId),
    ExpansionToken(ExpansionTokenId),
    HirSymbol(HirSymbolId),
    HirReference(HirReferenceId),
    SyntaxToken(SyntaxTokenEntityId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBlock {
    pub reason: SourceBlockReason,
    pub preferred_span: Option<SpanId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceBlockReason {
    GeneratedToken,
    DisplayOnly,
    NotWritable,
    AmbiguousContext,
    Unavailable(crate::SourceUnavailable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceChoice {
    Span(SpanId),
    FileRange(FileRange),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRangeResult<T> {
    Mapped(T),
    Blocked(SourceBlock),
    Unavailable(crate::SourceUnavailable),
}
