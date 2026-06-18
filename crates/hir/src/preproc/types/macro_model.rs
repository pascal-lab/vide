use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinition {
    pub id: MacroDefinitionId,
    pub file_id: FileId,
    pub name: SmolStr,
    pub params: Option<Vec<MacroDefinitionParam>>,
    pub body_tokens: Vec<SmolStr>,
    pub source_range: TextRange,
    pub directive_range: TextRange,
    pub name_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinitionParam {
    pub param_index: usize,
    pub name: Option<SmolStr>,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParamDefinition {
    pub macro_definition: MacroDefinition,
    pub param_index: usize,
    pub name: SmolStr,
    pub range: TextRange,
    pub param_range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParamReference {
    pub macro_definition: MacroDefinition,
    pub file_id: FileId,
    pub param_index: usize,
    pub token_index: usize,
    pub name: SmolStr,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParamReferenceDefinitions {
    pub references: Vec<MacroParamReference>,
    pub range: TextRange,
    pub definitions: Vec<MacroParamDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParamReferences {
    pub references: Vec<MacroParamReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUsage {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUsageResolution {
    pub usage: MacroUsage,
    pub definition: MacroDefinition,
    pub include_chain: Vec<IncludeChainEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeChainEntry {
    pub include_file_id: FileId,
    pub include_range: TextRange,
    pub included_file_id: FileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReference {
    pub file_id: FileId,
    pub name: SmolStr,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReferenceDefinitions {
    pub references: Vec<MacroReference>,
    pub range: TextRange,
    pub definitions: Vec<MacroDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCall {
    pub file_id: FileId,
    pub arguments: Vec<MacroArgument>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCallResolution {
    pub call: MacroCall,
    pub definition: MacroDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroArgument {
    pub argument_index: usize,
    pub range: Option<TextRange>,
}
