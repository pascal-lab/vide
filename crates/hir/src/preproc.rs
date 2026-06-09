use preproc::source::{
    CapabilityStatus, MacroIncludeTarget, PreprocSourceId, SourceEmittedTokenId,
    SourceEmittedTokenRange, SourceIncludeChainEntry, SourceIncludeStatus,
    SourceMacroArgument as SourceMacroArgumentFact, SourceMacroCall as SourceMacroCallFact,
    SourceMacroCallId, SourceMacroCallStatus as SourceMacroCallStatusFact,
    SourceMacroDefinition as SourceMacroDefinitionFact,
    SourceMacroExpansion as SourceMacroExpansionFact,
    SourceMacroExpansionDefinition as SourceMacroExpansionDefinitionFact, SourceMacroExpansionId,
    SourceMacroExpansionQuery as SourceMacroExpansionQueryFact,
    SourceMacroExpansionStatus as SourceMacroExpansionStatusFact,
    SourceMacroParam as SourceMacroParamFact, SourceMacroReference as SourceMacroReferenceFact,
    SourceMacroReferenceSite, SourceMacroResolution as SourceMacroResolutionFact,
    SourceMacroResolutionReason as SourceMacroResolutionReasonFact, SourcePreprocError,
    SourcePreprocUnavailable, SourceRange, SourceTokenProvenance as SourceTokenProvenanceFact,
};
use smol_str::SmolStr;
use utils::{
    line_index::{TextRange, TextSize},
    uniq_vec::UniqVec,
};
use vfs::FileId;

use crate::base_db::{
    project::{CompilationProfileId, Predefine},
    source_db::{
        MappedSourcePreprocModel, PreprocSourceMapError, PreprocSourceMapping, SourceFileKind,
        SourcePreprocContextStatus, SourcePreprocQueryError, SourceRootDb,
        workspace_preproc_model_file_ids,
    },
};

mod conditionals;
mod definitions;
mod expansion;
mod helpers;
mod includes;
mod predefines;
mod presentation;
mod reference_index;
mod reference_queries;
mod types;

use self::helpers::*;
pub use self::{
    conditionals::*, definitions::*, expansion::*, includes::*, presentation::*,
    reference_index::*, reference_queries::*, types::*,
};

#[cfg(test)]
mod tests;
