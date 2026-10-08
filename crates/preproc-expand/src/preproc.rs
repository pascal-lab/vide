use base_db::{
    project::{CompilationProfileId, Predefine},
    source_db::SourceFileKind,
};
use preproc::source::{
    MacroIncludeTarget, PreprocSourceId, SourceIncludeStatus, SourceMacroArgument, SourceMacroCall,
    SourceMacroDefinition, SourceMacroParam, SourceMacroReference, SourceMacroResolution,
    SourcePreprocError, SourcePreprocUnavailable, SourceRange,
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
        MappedSourcePreprocModel, PreprocSourceMapping, SourcePreprocQueryError,
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

/// The macro name of a predefine config string (`FOO=1` -> `FOO`).
pub(crate) fn predefine_name(predefine: &str) -> Option<SmolStr> {
    let name = predefine.split_once('=').map_or(predefine, |(name, _)| name);
    let name = name.trim().strip_prefix('`').unwrap_or(name.trim());
    if name.is_empty() { None } else { Some(SmolStr::new(name)) }
}

pub(crate) use self::helpers::mapping::definitions::map_macro_definition;
use self::helpers::*;
pub use self::{
    conditionals::*, definitions::*, diagnostics::*, expansion::*, includes::*, reference_index::*,
    reference_queries::*, types::*,
};

#[cfg(test)]
mod tests;
