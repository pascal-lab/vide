use ::preproc::source::{
    PreprocSourceId, SourceMacroCallId, SourceMacroDefinitionId, SourceMacroReferenceId,
    SourcePosition, SourcePreprocError, SourcePreprocModel, SourcePreprocUnavailable, SourceRange,
};
use base_db::{project::CompilationProfileId, source_db::SourceFileKind};
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use syntax::{
    SyntaxTreeOptions,
    preproc::{SourceBufferOrigin, Trace},
};
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    path_identity::PathIdentityIndex,
};
use vfs::{FileId, VfsPath};

use crate::db::{PreprocDb, syntax_tree_options_for_file};

mod context;
mod queries;
pub(crate) mod range_index;
mod source_map;
mod source_mapping;

#[cfg(not(test))]
use self::source_mapping::source_preproc_file_ids;
#[cfg(test)]
pub(super) use self::source_mapping::{materialized_predefine_text, source_preproc_file_ids};
use self::source_mapping::{shift_text_range, unshift_text_size};
pub use self::{
    context::{SourcePreprocContextIndex, SourcePreprocRelevantContexts},
    queries::{SourcePreprocQueryError, workspace_preproc_model_file_ids},
    range_index::MappedSourcePreprocModel,
    source_map::{
        PreprocManifestSource, PreprocSourceMap, PreprocSourceMapping, PreprocVirtualOrigin,
    },
    source_mapping::{manifest_predefine_name_range_in_text, preproc_virtual_predefines_path},
};
pub(super) use self::{
    context::{source_preproc_context_index_for_profile, source_preproc_contexts_for_file},
    queries::source_preproc_model,
};
pub(crate) use self::{
    queries::set_source_preproc_model_lru_capacity, source_mapping::manifest_predefine_name_range,
};
