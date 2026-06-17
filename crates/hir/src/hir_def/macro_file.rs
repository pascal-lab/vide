use utils::line_index::TextRange;
use vfs::FileId;

use crate::base_db::salsa;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct MacroFileId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct MacroFileLoc {
    pub call_file: FileId,
    pub call_range: TextRange,
}
