pub mod directive_index;
mod macro_db;

pub use macro_db::{
    FileMacroInput, MacroDb, MacroDbInput, MacroDefId, MacroDefinitionAtResult, MacroName,
    MacroPredefine, MacroProfileId, MacroQueryFailure, MacroReferencesResult, MacroSource,
    MacroUse, MacroUseId, MacroUseResolution, PredefineSource, SourceOrigin,
};
