use base_db::{salsa, source_root::SourceRootId};
use vfs::FileId;

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
