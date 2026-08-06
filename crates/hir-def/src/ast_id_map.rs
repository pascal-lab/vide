//! Per-file stable AST node identity.
//!
//! Every root-buffer node of a file's syntax tree gets a `SourceAstId`
//! assigned in depth-first preorder: a parent is numbered before its
//! children, and appending a node at the end of the tree never renumbers
//! existing nodes. The ids are file-local and deterministic for a given
//! parse, so they are the join key between the syntax tree and every
//! semantic layer (item tree, [`OwnerId`](crate::owner::OwnerId), body
//! source maps) without holding a tree or a [`SyntaxNodePtr`] inside the
//! semantic structures.
//!
//! The design doc ([`docs/lsp/hir-def-rearchitecture.md`](https://raw.githubusercontent.com/hjiaming/vide/main/docs/lsp/hir-def-rearchitecture.md))
//! sketches a BFS ordering, but the stability property it wants ("appending
//! a sibling never renumbers existing nodes") is only true of depth-first
//! preorder: in BFS, a new sibling at a shallower depth is numbered before
//! the existing deeper nodes it precedes. rust-analyzer has since moved to
//! content-hashed ids (kind + name + parent) for the same reason; if that
//! stability class is ever needed, the map can be rebuilt on top of
//! `SourceAstId` without changing consumers.
//!
//! Nodes that have no stable position in the file's display coordinates
//! (syntax from included buffers) are not numbered and yield `None`; this is
//! the same boundary the source maps draw with
//! [`SourceAst`](crate::source_map::SourceAst).
//!
//! Macro-expanded nodes report the macro call site as their display range, so
//! [`SyntaxNodePtr`] is not a unique identity inside one expansion — the same
//! caveat that already applies to [`SyntaxNodePtr`] itself. In practice only
//! one macro call expands at a given call-site range, so collisions are
//! limited to a single expansion emitting two nodes of the same kind.

use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use syntax::{SyntaxNode, SyntaxTree, has_text_range::HasTextRange, ptr::SyntaxNodePtr};
use triomphe::Arc;

use crate::db::HirDefDb;

/// A stable, file-local index of a syntax node in breadth-first preorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceAstId(pub u32);

impl SourceAstId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// The node table behind [`SourceAstId`]: `id -> SyntaxNodePtr` with the
/// reverse lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstIdMap {
    nodes: Vec<SyntaxNodePtr>,
    by_ptr: FxHashMap<SyntaxNodePtr, SourceAstId>,
}

impl AstIdMap {
    /// Builds the map in depth-first preorder: each node is numbered on
    /// entry, after its parent and before any of its children.
    pub(crate) fn from_source(tree: &SyntaxTree) -> Self {
        let mut nodes = Vec::new();
        let mut by_ptr = FxHashMap::default();
        let Some(root) = tree.root() else {
            return Self { nodes, by_ptr };
        };

        for event in root.node_preorder() {
            let syntax::WalkEvent::Enter(node) = event else {
                continue;
            };
            if node.text_range().is_some() {
                let ptr = SyntaxNodePtr::from_node(node);
                let id = SourceAstId(u32::try_from(nodes.len()).unwrap_or(u32::MAX));
                by_ptr.insert(ptr, id);
                nodes.push(ptr);
            }
        }

        Self { nodes, by_ptr }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The node pointer for `id`.
    pub fn ptr(&self, id: SourceAstId) -> Option<SyntaxNodePtr> {
        self.nodes.get(id.0 as usize).copied()
    }

    /// The id of a root-buffer node, when it has one.
    pub fn id_of_node(&self, node: SyntaxNode<'_>) -> Option<SourceAstId> {
        (node.text_range().is_some())
            .then(|| SyntaxNodePtr::from_node(node))
            .and_then(|ptr| self.id_of_ptr(ptr))
    }

    /// The id of a node pointer, when the node is in the map.
    pub fn id_of_ptr(&self, ptr: SyntaxNodePtr) -> Option<SourceAstId> {
        self.by_ptr.get(&ptr).copied()
    }
}

#[salsa::tracked(lru = 1024, returns(clone))]
pub(crate) fn ast_id_map(db: &dyn HirDefDb, file_id: HirFileId, _key: ()) -> Arc<AstIdMap> {
    Arc::new(AstIdMap::from_source(&db.parse(file_id)))
}

pub(crate) fn set_ast_id_map_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    ast_id_map::set_lru_capacity(db, capacity);
}

#[cfg(test)]
mod tests {
    use syntax::SyntaxKind;

    use super::*;

    fn parse(text: &str) -> SyntaxTree {
        SyntaxTree::from_text(text, "test.sv", "test.sv")
    }

    #[test]
    fn preorder_orders_parents_before_children() {
        let text = "module m; wire a; wire b; endmodule\n";
        let map = AstIdMap::from_source(&parse(text));
        let tree = parse(text);
        let root = tree.root().unwrap();

        // Collect (kind, id) for every numbered node in tree order and check
        // that each node's id is larger than any ancestor's id.
        let mut ids = Vec::new();
        let mut stack: Vec<u32> = Vec::new();
        for event in root.node_preorder() {
            match event {
                syntax::WalkEvent::Enter(node) => {
                    if let Some(id) = map.id_of_node(node) {
                        if let Some(parent) = stack.last() {
                            assert!(
                                *parent < id.0,
                                "parent must be numbered before child (parent {parent}, child {id:?})"
                            );
                        }
                        stack.push(id.0);
                        ids.push((node.kind(), id));
                    }
                }
                syntax::WalkEvent::Leave(node) => {
                    if map.id_of_node(node).is_some() {
                        stack.pop();
                    }
                }
            }
        }

        assert_eq!(ids.first(), Some(&(SyntaxKind::COMPILATION_UNIT, SourceAstId(0))));
        assert!(
            ids.iter().any(|(kind, _)| *kind == SyntaxKind::MODULE_DECLARATION),
            "module nodes must be numbered: {ids:?}"
        );
    }

    #[test]
    fn appending_a_member_does_not_renumber_existing_nodes() {
        let before = "module m; wire a; endmodule\n";
        let after = "module m; wire a; wire b; endmodule\n";
        let before_map = AstIdMap::from_source(&parse(before));
        let after_map = AstIdMap::from_source(&parse(after));

        let before_ids = collect_kind_ids(&before_map, &parse(before));
        let after_ids = collect_kind_ids(&after_map, &parse(after));

        // Every node that exists in both trees keeps its id.
        for ((kind_before, id_before), (kind_after, id_after)) in
            before_ids.iter().zip(after_ids.iter())
        {
            assert_eq!(kind_before, kind_after);
            assert_eq!(id_before, id_after, "{kind_before:?} must keep its id");
        }
        assert!(after_ids.len() > before_ids.len());
    }

    #[test]
    fn id_of_ptr_roundtrips() {
        let text = "module m; endmodule\n";
        let tree = parse(text);
        let map = AstIdMap::from_source(&tree);
        let root = tree.root().unwrap();

        let id = map.id_of_node(root).expect("root must be numbered");
        assert_eq!(map.ptr(id), Some(SyntaxNodePtr::from_node(root)));
        assert_eq!(map.id_of_ptr(SyntaxNodePtr::from_node(root)), Some(id));
    }

    fn collect_kind_ids(map: &AstIdMap, tree: &SyntaxTree) -> Vec<(SyntaxKind, SourceAstId)> {
        let mut out = Vec::new();
        for event in tree.root().unwrap().node_preorder() {
            if let syntax::WalkEvent::Enter(node) = event
                && let Some(id) = map.id_of_node(node)
            {
                out.push((node.kind(), id));
            }
        }
        out
    }
}
