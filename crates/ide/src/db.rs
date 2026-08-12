use base_db::salsa;
use vfs::FileId;

// Salsa 0.28 tracked functions require salsa-struct arguments. `FileId` is a
// plain integer, so it needs an interned wrapper to serve as the key for
// `line_index`. All other ide functions are untracked and accept `FileId`
// directly.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct SourceFileQueryKey {
    #[returns(copy)]
    pub file_id: FileId,
}

pub mod apply_change;
pub mod line_index_db;
pub mod root_db;
pub mod workspace_symbol_index_db;