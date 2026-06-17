use ::preproc::source::{
    PreprocSourceId, SourceEmittedTokenId, SourceEmittedTokenRange, SourceMacroCallId,
    SourceMacroExpansionId, SourceMacroReferenceId, SourcePosition, SourcePreprocError,
    SourcePreprocModel, SourcePreprocUnavailable, SourceRange, SourceTokenOrigin,
};
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use syntax::{SourceBufferOrigin, SyntaxTreeOptions, preproc::Trace};
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    path_identity::PathIdentityIndex,
    uniq_vec::UniqVec,
};
use vfs::{FileId, VfsPath};

use super::{SourceFileKind, SourceRootDb, path_file_ids, syntax_tree_options_for_file};
use crate::base_db::project::CompilationProfileId;

mod context;
mod queries;
mod range_index;
mod source_map;
mod source_mapping;

pub(crate) use self::queries::workspace_preproc_model_file_ids;
#[cfg(not(test))]
use self::source_mapping::source_preproc_file_ids;
use self::source_mapping::{
    display_only_expansion_source_buffer_error, emitted_range_from_token_ranges,
    record_expansion_display_texts, shift_text_range, unshift_text_size,
};
#[cfg(test)]
pub(super) use self::source_mapping::{materialized_predefine_text, source_preproc_file_ids};
pub use self::{
    context::{
        SourcePreprocContextIndex, SourcePreprocContextStatus, SourcePreprocRelevantContexts,
    },
    queries::SourcePreprocQueryError,
    range_index::MappedSourcePreprocModel,
    source_map::{
        PreprocExpansionDisplay, PreprocExpansionMapping, PreprocExpansionSourceBuffer,
        PreprocManifestSource, PreprocSourceMap, PreprocSourceMapError, PreprocSourceMapping,
        PreprocSpeculativeUniverseId, PreprocVirtualOrigin,
    },
    source_mapping::{
        preproc_virtual_builtin_path, preproc_virtual_expansion_path,
        preproc_virtual_predefines_path, preproc_virtual_speculative_path,
    },
};
pub(super) use self::{
    context::{source_preproc_context_index_for_profile, source_preproc_contexts_for_file},
    queries::source_preproc_model,
};
