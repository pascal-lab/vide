use preproc_expand::file::HirFileId;
use syntax::{SyntaxKind, ptr::SyntaxNodePtr};
use triomphe::Arc;
use utils::text_edit::TextRange;

use crate::{db::HirDefDb, item_tree::ItemTreeId};

/// A source representation of an item in an expanded HIR file.
///
/// This is deliberately separate from [`ItemTreeId`]. An item may remain
/// semantically useful when its syntax node has no stable root-buffer range,
/// for example when it originates from an include or macro expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceOrigin {
    file_id: HirFileId,
    node: Option<SyntaxNodePtr>,
    kind: Option<SyntaxKind>,
    full_range: Option<TextRange>,
    focus_range: Option<TextRange>,
}

impl SourceOrigin {
    pub(crate) fn new(
        file_id: HirFileId,
        node: Option<SyntaxNodePtr>,
        kind: Option<SyntaxKind>,
        full_range: Option<TextRange>,
        focus_range: Option<TextRange>,
    ) -> Self {
        Self { file_id, node, kind, full_range, focus_range }
    }

    pub fn file_id(self) -> HirFileId {
        self.file_id
    }

    pub fn node(self) -> Option<SyntaxNodePtr> {
        self.node
    }

    pub fn kind(self) -> Option<SyntaxKind> {
        self.kind
    }

    pub fn full_range(self) -> Option<TextRange> {
        self.full_range
    }

    pub fn focus_range(self) -> Option<TextRange> {
        self.focus_range
    }

    pub fn focus_or_full_range(self) -> Option<TextRange> {
        self.focus_range.or(self.full_range)
    }

    pub fn is_navigable(self) -> bool {
        self.node.is_some() && self.full_range.is_some()
    }
}

/// Maps ItemTree ids to all source-level information needed by editor
/// features. The mapping is one-to-one for now, while the value type already
/// permits a future multi-origin projection without changing callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProjection {
    origins: Vec<Option<SourceOrigin>>,
}

impl SourceProjection {
    pub(crate) fn new(origins: Vec<Option<SourceOrigin>>) -> Self {
        Self { origins }
    }

    pub fn origin(&self, item: ItemTreeId) -> Option<SourceOrigin> {
        self.origins.get(item.raw() as usize).copied().flatten()
    }

    pub fn origins(&self) -> impl Iterator<Item = (ItemTreeId, SourceOrigin)> + '_ {
        self.origins.iter().enumerate().filter_map(|(raw, origin)| {
            origin.map(|origin| (ItemTreeId::from_raw(raw as u32), origin))
        })
    }

    pub fn len(&self) -> usize {
        self.origins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn source_projection(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    _key: (),
) -> Arc<SourceProjection> {
    let tree = db.parse(file_id);
    Arc::new(crate::item_tree::build_source_projection(file_id, &tree))
}

pub(crate) fn set_source_projection_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    source_projection::set_lru_capacity(db, capacity);
}
