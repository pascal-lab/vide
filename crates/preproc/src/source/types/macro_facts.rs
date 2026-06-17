use smol_str::SmolStr;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocEventRecord {
    pub event_id: SourcePreprocEventId,
    pub kind: MacroEventKind,
    pub range: SourceRange,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroDefine {
    pub event_id: SourcePreprocEventId,
    pub identity: Option<MacroDefinitionId>,
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
    pub event_id: SourcePreprocEventId,
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroInclude {
    pub event_id: SourcePreprocEventId,
    pub target: MacroIncludeTarget,
    pub target_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroConditional {
    pub event_id: SourcePreprocEventId,
    pub kind: MacroConditionalKind,
    pub expr: Vec<SourceMacroToken>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroUsage {
    pub event_id: SourcePreprocEventId,
    pub identity: Option<MacroCallId>,
    pub definition_identity: Option<MacroDefinitionId>,
    pub expansion_identity: Option<MacroExpansionId>,
    pub parent_expansion_identity: Option<MacroExpansionId>,
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub arguments: Vec<SourceMacroActualArgument>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroActualArgument {
    pub argument_index: usize,
    pub argument_range: Option<SourceRange>,
    pub tokens: Vec<SourceMacroToken>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroToken {
    pub raw: SmolStr,
    pub value: SmolStr,
    pub range: Option<SourceRange>,
}
