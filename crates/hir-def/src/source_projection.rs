use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use syntax::{SyntaxKind, ptr::SyntaxNodePtr};
use triomphe::Arc;
use utils::text_edit::TextRange;

use crate::{
    ast_id_map::{SourceAstId, SyntaxFileId},
    db::HirDefDb,
};

/// Current source representation for a canonical AST identity. Navigability
/// remains independent from semantic identity.
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

/// Maps canonical AST identities to editor-facing source data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProjection {
    origins: FxHashMap<SourceAstId, SourceOrigin>,
}

impl SourceProjection {
    pub(crate) fn new(origins: FxHashMap<SourceAstId, SourceOrigin>) -> Self {
        Self { origins }
    }

    pub fn origin(&self, ast_id: SourceAstId) -> Option<SourceOrigin> {
        self.origins.get(&ast_id).copied()
    }

    pub fn node<'tree>(
        &self,
        ast_id: SourceAstId,
        tree: &'tree syntax::SyntaxTree,
    ) -> Option<syntax::SyntaxNode<'tree>> {
        self.origin(ast_id)?.node()?.to_node(tree)
    }

    pub fn origins(&self) -> impl Iterator<Item = (SourceAstId, SourceOrigin)> + '_ {
        self.origins.iter().map(|(id, origin)| (*id, *origin))
    }

    pub fn len(&self) -> usize {
        self.origins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn source_projection(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<SourceProjection> {
    let file_id = file.hir_file(db);
    let tree = db.parse(file_id);
    let ast_ids = crate::ast_id_map::ast_id_map(db, file);
    Arc::new(crate::item_tree::build_source_projection(file_id, &tree, &ast_ids))
}

pub(crate) fn set_source_projection_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    source_projection::set_lru_capacity(db, capacity);
}
