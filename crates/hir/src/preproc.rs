use preproc::source::{
    CapabilityStatus, MacroIncludeTarget, PreprocSourceId, SourceEmittedTokenId,
    SourceEmittedTokenRange, SourceIncludeChainEntry, SourceIncludeStatus,
    SourceMacroArgument as SourceMacroArgumentFact, SourceMacroCall as SourceMacroCallFact,
    SourceMacroCallId, SourceMacroCallStatus as SourceMacroCallStatusFact,
    SourceMacroDefinition as SourceMacroDefinitionFact,
    SourceMacroExpansion as SourceMacroExpansionFact,
    SourceMacroExpansionDefinition as SourceMacroExpansionDefinitionFact, SourceMacroExpansionId,
    SourceMacroExpansionStatus as SourceMacroExpansionStatusFact,
    SourceMacroResolution as SourceMacroResolutionFact,
    SourceMacroResolutionReason as SourceMacroResolutionReasonFact, SourcePreprocError,
    SourcePreprocUnavailable, SourceRange, SourceTokenProvenance as SourceTokenProvenanceFact,
};
use smol_str::SmolStr;
use utils::{
    line_index::{TextRange, TextSize},
    uniq_vec::UniqVec,
};
use vfs::FileId;

use crate::base_db::source_db::{
    MappedSourcePreprocModel, PreprocSourceMapError, PreprocSourceMapping, SourceFileKind,
    SourcePreprocContextStatus, SourcePreprocQueryError, SourceRootDb,
};

mod conditionals;
mod definitions;
mod expansion;
mod helpers;
mod includes;
mod predefines;
mod types;

use self::helpers::*;
pub use self::{
    conditionals::inactive_branches,
    definitions::visible_macro_names_at,
    expansion::{
        macro_call_resolutions_in_range, recursive_macro_expansion_provenance_for_source_graph_call,
    },
    includes::include_directive_at,
    types::*,
};

#[cfg(test)]
mod tests;
