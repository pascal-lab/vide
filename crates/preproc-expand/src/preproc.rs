use base_db::{
    project::{CompilationProfileId, Predefine},
    source_db::SourceFileKind,
};
use preproc::source::{
    MacroIncludeTarget, PreprocSourceId, SourceIncludeChainEntry, SourceIncludeStatus,
    SourceMacroArgument, SourceMacroCall, SourceMacroDefinition, SourceMacroParam,
    SourceMacroReference, SourceMacroReferenceSite, SourceMacroResolution, SourcePreprocError,
    SourcePreprocUnavailable, SourceRange,
};
use smol_str::SmolStr;
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    uniq_vec::UniqVec,
};
use vfs::FileId;

pub(crate) use self::reference_index::macro_reference_index_for_profile_query;
use crate::{
    db::PreprocDb,
    source_db::{
        MappedSourcePreprocModel, PreprocSourceMapError, PreprocSourceMapping,
        SourcePreprocContextStatus, SourcePreprocQueryError, workspace_preproc_model_file_ids,
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

pub(crate) use self::helpers::mapping::definitions::map_macro_definition;
use self::helpers::*;
pub use self::{
    conditionals::*, definitions::*, diagnostics::*, expansion::*, includes::*, reference_index::*,
    reference_queries::*, types::*,
};

#[cfg(test)]
mod tests;
