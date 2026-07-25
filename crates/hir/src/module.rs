//! High-level `Module` handle.
//!
//! Wraps a located module identifier (`InFile<LocalModuleId>`) without
//! exposing the arena index or the `InFile`/container types. IDE code reads a
//! module's name, kind, and source location through this facade.

use smol_str::SmolStr;

use crate::File;

/// A module declaration located in some [`File`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Module {
    file: File,
    local: hir_def::module::LocalModuleId,
}
impl Module {
    pub(crate) fn new(file: File, local: hir_def::module::LocalModuleId) -> Self {
        Self { file, local }
    }

    /// Facade-internal: the arena index backing this module.
    pub(crate) fn local_id(self) -> hir_def::module::LocalModuleId {
        self.local
    }

    /// The file this module is declared in.
    pub fn file(self) -> File {
        self.file
    }

    /// The module's declared name, if any. Anonymous modules (e.g. generate
    /// regions lowered as modules) return `None`.
    pub fn name(self, db: &dyn hir_def::db::HirDefDb) -> Option<SmolStr> {
        let lowered = db.hir_file_with_source_map(self.file.into_inner());
        lowered.modules[self.local].name.clone()
    }
}

/// The source location (`file` + `range`) of a HIR item.
///
/// Replaces the raw `Lowered<HirFile>` source-map iteration that IDE code used
/// to perform by hand. `range` is the full syntactic extent of the item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    file: File,
    range: utils::text_edit::TextRange,
}

impl Source {
    pub(crate) fn new(file: File, range: utils::text_edit::TextRange) -> Self {
        Self { file, range }
    }

    pub fn file(self) -> File {
        self.file
    }

    pub fn range(self) -> utils::text_edit::TextRange {
        self.range
    }
}