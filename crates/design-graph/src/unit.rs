use smol_str::SmolStr;
use utils::line_index::TextRange;
use vfs::FileId;

/// Workspace design-unit identity. A value type; not interned.
///
/// `ordinal` is the occurrence of `(file, name, kind)` in that file's
/// unexpanded decls, then any generated supplement, starting at 0.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnitId {
    pub file: FileId,
    pub name: SmolStr,
    pub kind: UnitKind,
    pub ordinal: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitKind {
    Module,
    Interface,
    Package,
    Program,
    Checker,
    Covergroup,
}

impl UnitKind {
    /// Legal target of a hierarchy instantiation. Not Package / Checker /
    /// Covergroup.
    pub fn is_hierarchy_target(self) -> bool {
        matches!(self, Self::Module | Self::Interface | Self::Program)
    }

    pub fn is_package(self) -> bool {
        matches!(self, Self::Package)
    }

    pub fn is_design_unit(self) -> bool {
        true
    }
}

/// Display facts for a node. Not identity.
///
/// `name_range` / `header_range` are display coordinates in `file_text`.
/// Absent when extract could not assign a single-buffer range, or when the
/// node is generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitNode {
    pub id: UnitId,
    pub name_range: Option<TextRange>,
    pub header_range: Option<TextRange>,
    pub header_fingerprint: u64,
    pub origin: UnitOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitOrigin {
    /// Unexpanded-tree source declaration. Ranges may slice `file_text`.
    Source,
    /// Paid authoritative tree, name token is not `TokenOrigin::Source`.
    Generated,
}

impl Default for UnitOrigin {
    fn default() -> Self {
        Self::Source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstantiationRole {
    /// `ast::HierarchyInstantiation` only.
    Hierarchy,
    /// `ast::CheckerInstantiation` only.
    Checker,
}
