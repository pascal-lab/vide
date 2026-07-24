use preproc::source::{
    MacroIncludeTarget, PreprocSourceId, SourceEmittedTokenId, SourceEmittedTokenRange,
    SourceIncludeChainEntry, SourceIncludeStatus, SourceMacroArgument, SourceMacroCall,
    SourceMacroCallId, SourceMacroDefinition, SourceMacroExpansion, SourceMacroParam,
    SourceMacroReference, SourceMacroReferenceSite, SourceMacroResolution, SourcePreprocError,
    SourcePreprocUnavailable, SourceRange, SourceTokenOrigin,
};
use smol_str::SmolStr;
use utils::{
    line_index::{TextRange, TextSize},
    uniq_vec::UniqVec,
};
use vfs::FileId;

use crate::{
    base_db::{
        project::{CompilationProfileId, Predefine},
        source_db::{
            MappedSourcePreprocModel, PreprocSourceMapError, PreprocSourceMapping, SourceFileKind,
            SourcePreprocContextStatus, SourcePreprocQueryError, SourceRootDb,
            workspace_preproc_model_file_ids,
        },
    },
    db::HirDb,
};

use triomphe::Arc;

#[salsa::query_group(PreprocDbStorage)]
pub trait PreprocDb: SourceRootDb {
    #[salsa::invoke(reference_index::macro_reference_index_for_profile_query)]
    fn macro_reference_index_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<MacroReferenceIndex>;
}

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

pub(crate) use self::helpers::mapping::definitions::map_macro_definition;
use self::helpers::*;
pub use self::{
    conditionals::*, definitions::*, diagnostics::*, expansion::*, includes::*, reference_index::*,
    reference_queries::*, types::*,
};

#[cfg(test)]
mod tests;
