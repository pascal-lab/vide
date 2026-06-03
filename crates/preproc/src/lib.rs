#[cfg(test)]
mod architecture_tests;
pub mod directive_index;
mod macro_db;

pub use macro_db::{
    FileMacroInput, IncludeTargetAtResult, LiteralIncludeInput, MacroDb, MacroDbInput, MacroDefId,
    MacroDefinitionAtResult, MacroName, MacroPredefine, MacroProfileId, MacroQueryFailure,
    MacroReference, MacroReferencesResult, MacroSource, MacroUse, MacroUseId, MacroUseResolution,
    PredefineSource, SourceOrigin,
};
