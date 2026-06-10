use crate::ids::{
    ExpansionTokenId, HirReferenceId, HirSymbolId, InactiveRegionId, IncludeDirectiveId,
    MacroCallId, MacroDefinitionId, MacroParamDefinitionId, MacroParamReferenceId,
    MacroReferenceId, SyntaxTokenEntityId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceEntity {
    MacroDefinition(MacroDefinitionId),
    MacroReference(MacroReferenceId),
    MacroCall(MacroCallId),
    MacroParamDefinition(MacroParamDefinitionId),
    MacroParamReference(MacroParamReferenceId),
    IncludeDirective(IncludeDirectiveId),
    InactiveRegion(InactiveRegionId),
    ExpansionToken(ExpansionTokenId),
    HirSymbol(HirSymbolId),
    HirReference(HirReferenceId),
    SyntaxToken(SyntaxTokenEntityId),
}
