use ide_db::root_db::RootDb;
use line_index::TextRange;
use smol_str::SmolStr;
use vfs::vfs::FileId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NavTarget {
    pub file_id: FileId,
    pub full_range: TextRange,
    pub focus_range: Option<TextRange>,

    pub name: SmolStr,
    pub kind: Option<SymbolKind>,
    pub container_name: Option<SmolStr>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolKind {
    Module,
    Data,
}

pub(crate) trait ToNav {
    fn to_nav(&self, db: &RootDb) -> NavTarget;
}

pub(crate) trait TryToNav {
    fn try_to_nav(&self, db: &RootDb) -> Option<NavTarget>;
}
