use base_db::{salsa, source_root::SourceRootId};
use vfs::FileId;

// Salsa 0.28 tracked functions require salsa-struct arguments. `FileId` and
// `SourceRootId` are plain integers, so they need interned wrappers to serve
// as tracked-query keys (line index, module/semantic index queries).
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct SourceFileQueryKey {
    #[returns(copy)]
    pub file_id: FileId,
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct SourceRootQueryKey {
    #[returns(copy)]
    pub source_root_id: SourceRootId,
}

pub mod apply_change;
pub mod line_index_db;
pub mod root_db;
pub mod workspace_symbol_index_db;
