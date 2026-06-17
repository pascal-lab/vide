use preproc::source::{
    MacroIncludeTarget, PreprocSourceId, SourceEmittedTokenId, SourceEmittedTokenRange,
    SourceIncludeChainEntry, SourceIncludeStatus, SourceMacroArgument, SourceMacroCall,
    SourceMacroCallId, SourceMacroDefinition, SourceMacroExpansion, SourceMacroExpansionId,
    SourceMacroExpansionQuery, SourceMacroParam, SourceMacroReference, SourceMacroReferenceSite,
    SourceMacroResolution, SourceMacroResolutionReason, SourcePreprocError,
    SourcePreprocUnavailable, SourceRange, SourceTokenOrigin,
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
mod diagnostics;
mod expansion;
mod helpers;
mod includes;
mod predefines;
mod reference_index;
mod reference_queries;
mod types;

use self::helpers::*;
pub use self::{
    conditionals::*, definitions::*, diagnostics::*, expansion::*, includes::*, reference_index::*,
    reference_queries::*, types::*,
};

#[cfg(test)]
mod tests;
