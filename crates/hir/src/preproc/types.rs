use std::collections::BTreeMap;

use preproc::source::{
    SourceEmittedTokenRange, SourceIncludeDirectiveId, SourceMacroCallId, SourceMacroDefinitionId,
    SourceMacroExpansionId, SourceMacroReferenceId, SourcePreprocError, SourcePreprocUnavailable,
};
use smol_str::SmolStr;
use utils::{
    line_index::{TextRange, TextSize},
    uniq_vec::UniqVec,
};
use vfs::{FileId, VfsPath};

use crate::base_db::source_db::{
    PreprocSourceMapError, PreprocVirtualOrigin, SourcePreprocQueryError,
};

mod common;
mod diagnostics;
mod includes;
mod macro_model;
mod reference_index;

pub use self::{common::*, diagnostics::*, includes::*, macro_model::*, reference_index::*};
