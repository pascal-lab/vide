//! Per-file source AST identity.
//!
//! `SourceAstId` is a stable path identity, not an arena ordinal. The path is
//! made from syntax kind, parent path, and the occurrence among direct
//! siblings of the same kind. Existing item/header nodes therefore keep their
//! id when an unrelated body grows, while a structural insertion is allowed
//! to change the ids of later siblings of the same kind.

use std::hash::{Hash, Hasher};

use preproc_expand::file::HirFileId;
use rustc_hash::{FxHashMap, FxHasher};
use syntax::{
    SyntaxKind, SyntaxNode, SyntaxTree, has_text_range::HasTextRange, ptr::SyntaxNodePtr,
};
use triomphe::Arc;

use crate::db::HirDefDb;

/// Database-interned identity of a parsed HIR file.
///
/// `HirFileId` is a plain value from the preprocessing layer. Interning it at
/// this seam gives every file-derived query one real Salsa key instead of the
/// tuple-interning `_key: ()` workaround.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct SyntaxFileId {
    #[returns(copy)]
    pub hir_file: HirFileId,
}

/// A file-local identity for one syntax node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceAstId(pub u128);

impl SourceAstId {
    pub const fn from_raw(raw: u128) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u128 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct StableSegment {
    kind: SyntaxKind,
    occurrence: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct StablePath(Vec<StableSegment>);

impl StablePath {
    fn child(&self, kind: SyntaxKind, occurrence: u32) -> Self {
        let mut path = self.0.clone();
        path.push(StableSegment { kind, occurrence });
        Self(path)
    }
}

/// The node table behind [`SourceAstId`]. Unique root-buffer nodes retain an
/// O(1) pointer; nodes with duplicate display pointers (macro expansion) are
/// resolved by their syntax-node identity and structural path. `by_preorder`
/// records each id's depth-first preorder index so source positions can be
/// compared within one revision without touching revision-local ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstIdMap {
    nodes: FxHashMap<SourceAstId, SyntaxNodePtr>,
    by_ptr: FxHashMap<SyntaxNodePtr, SourceAstId>,
    by_identity: FxHashMap<usize, SourceAstId>,
    by_path: FxHashMap<StablePath, SourceAstId>,
    paths: FxHashMap<SourceAstId, StablePath>,
    by_preorder: FxHashMap<SourceAstId, u32>,
}

impl AstIdMap {
    pub(crate) fn from_source(tree: &SyntaxTree) -> Self {
        let mut candidates = Vec::new();
        let mut paths: Vec<StablePath> = Vec::new();
        let mut child_counts: Vec<FxHashMap<SyntaxKind, u32>> = Vec::new();
        let root = tree.root();

        for event in root.node_preorder() {
            match event {
                syntax::WalkEvent::Enter(node) => {
                    let path = next_path(&paths, &mut child_counts, node.kind());
                    let ptr = node.text_range().map(|_| SyntaxNodePtr::from_node(node));
                    candidates.push((path.clone(), ptr, node.identity()));
                    paths.push(path);
                    child_counts.push(FxHashMap::default());
                }
                syntax::WalkEvent::Leave(_) => {
                    paths.pop();
                    child_counts.pop();
                }
            }
        }

        let mut nodes = FxHashMap::default();
        let mut by_ptr = FxHashMap::default();
        let mut by_identity = FxHashMap::default();
        let mut by_path = FxHashMap::default();
        let mut paths_by_id = FxHashMap::default();
        let mut by_preorder = FxHashMap::default();
        let mut ptr_counts = FxHashMap::<SyntaxNodePtr, u32>::default();
        for ptr in candidates.iter().filter_map(|(_, ptr, _)| *ptr) {
            *ptr_counts.entry(ptr).or_default() += 1;
        }
        let mut used = FxHashMap::<SourceAstId, StablePath>::default();
        for (preorder, (path, ptr, identity)) in candidates.into_iter().enumerate() {
            let mut salt = 0;
            let id = loop {
                let id = if path.0.is_empty() { SourceAstId(0) } else { stable_id(&path, salt) };
                match used.get(&id) {
                    None => break id,
                    Some(existing) if existing == &path => break id,
                    Some(_) => salt += 1,
                }
            };
            used.insert(id, path.clone());
            paths_by_id.insert(id, path.clone());
            by_identity.insert(identity, id);
            by_path.insert(path, id);
            by_preorder.insert(id, preorder as u32);
            if let Some(ptr) = ptr.filter(|ptr| ptr_counts.get(ptr) == Some(&1)) {
                nodes.insert(id, ptr);
                by_ptr.insert(ptr, id);
            }
        }
        Self { nodes, by_ptr, by_identity, by_path, paths: paths_by_id, by_preorder }
    }

    pub fn len(&self) -> usize {
        self.by_path.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    pub fn ptr(&self, id: SourceAstId) -> Option<SyntaxNodePtr> {
        self.nodes.get(&id).copied()
    }

    /// Depth-first preorder index of a node within its file. Comparable only
    /// within one revision; the stable path identity is the cross-revision
    /// key while this ordinal orders nodes by source position.
    pub fn preorder(&self, id: SourceAstId) -> Option<u32> {
        self.by_preorder.get(&id).copied()
    }

    pub fn node<'tree>(
        &self,
        id: SourceAstId,
        tree: &'tree SyntaxTree,
    ) -> Option<SyntaxNode<'tree>> {
        if let Some(node) = self.ptr(id).and_then(|ptr| ptr.to_node(tree)) {
            return Some(node);
        }
        let target_path = self.paths.get(&id)?;
        let root = tree.root();
        let mut paths: Vec<StablePath> = Vec::new();
        let mut child_counts: Vec<FxHashMap<SyntaxKind, u32>> = Vec::new();
        for event in root.node_preorder() {
            match event {
                syntax::WalkEvent::Enter(node) => {
                    let path = next_path(&paths, &mut child_counts, node.kind());
                    if &path == target_path {
                        return Some(node);
                    }
                    paths.push(path);
                    child_counts.push(FxHashMap::default());
                }
                syntax::WalkEvent::Leave(_) => {
                    paths.pop();
                    child_counts.pop();
                }
            }
        }
        None
    }

    pub fn id_of_node(&self, node: SyntaxNode<'_>) -> Option<SourceAstId> {
        self.by_identity.get(&node.identity()).copied().or_else(|| {
            node.text_range()
                .map(|_| SyntaxNodePtr::from_node(node))
                .and_then(|ptr| self.id_of_ptr(ptr))
        })
    }

    pub fn id_of_node_in_tree(
        &self,
        tree: &SyntaxTree,
        target: SyntaxNode<'_>,
    ) -> Option<SourceAstId> {
        let root = tree.root();
        let mut target_root = target;
        while let Some(parent) = target_root.parent() {
            target_root = parent;
        }
        (target_root == root).then(|| self.by_identity.get(&target.identity()).copied()).flatten()
    }

    pub fn id_of_ptr(&self, ptr: SyntaxNodePtr) -> Option<SourceAstId> {
        self.by_ptr.get(&ptr).copied()
    }
}

fn next_path(
    paths: &[StablePath],
    child_counts: &mut [FxHashMap<SyntaxKind, u32>],
    kind: SyntaxKind,
) -> StablePath {
    let Some(parent) = paths.last() else {
        return StablePath(Vec::new());
    };
    let count = child_counts
        .last_mut()
        .expect("every syntax node has a child-count frame")
        .entry(kind)
        .and_modify(|count| *count += 1)
        .or_insert(0);
    parent.child(kind, *count)
}
fn stable_id(path: &StablePath, salt: u64) -> SourceAstId {
    let mut hi = FxHasher::default();
    path.hash(&mut hi);
    salt.hash(&mut hi);
    let mut lo = FxHasher::default();
    0x9e37_79b9_u64.hash(&mut lo);
    path.hash(&mut lo);
    salt.hash(&mut lo);
    SourceAstId((hi.finish() as u128) << 64 | lo.finish() as u128)
}

#[salsa::tracked(lru = 1024, returns(clone))]
pub(crate) fn ast_id_map(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<AstIdMap> {
    Arc::new(AstIdMap::from_source(&db.parse(file.hir_file(db))))
}

pub(crate) fn set_ast_id_map_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    ast_id_map::set_lru_capacity(db, capacity);
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use syntax::SyntaxKind;

    use super::*;

    fn parse(text: &str) -> SyntaxTree {
        SyntaxTree::from_text(text, "test.sv", "test.sv")
    }

    #[test]
    fn stable_ids_are_unique_and_round_trip() {
        let tree = parse("module m; wire a; wire b; endmodule\n");
        let map = AstIdMap::from_source(&tree);
        let root = tree.root();
        assert_eq!(map.id_of_node(root), Some(SourceAstId(0)));

        let ids = collect_kind_ids(&map, &tree);
        let unique: HashSet<_> = ids.values().flatten().copied().collect();
        assert_eq!(unique.len(), map.len());
        assert!(ids.contains_key(&SyntaxKind::MODULE_DECLARATION));
        for id in unique {
            let node = map.node(id, &tree).expect("every AST id must resolve");
            assert_eq!(map.id_of_node(node), Some(id));
        }
    }

    #[test]
    fn preorder_ordinals_follow_source_order() {
        let tree = parse("module m; wire a; wire b; endmodule\n");
        let map = AstIdMap::from_source(&tree);
        let root = tree.root();
        assert_eq!(map.preorder(SourceAstId(0)), Some(0));

        let mut ordinals = Vec::new();
        for event in root.node_preorder() {
            let syntax::WalkEvent::Enter(node) = event else { continue };
            let id = map.id_of_node(node).expect("every node has an id");
            ordinals.push((map.preorder(id).expect("every id has a preorder"), id));
        }
        // Preorder ordinals are strictly increasing over a preorder walk.
        let mut sorted = ordinals.clone();
        sorted.sort_by_key(|(ordinal, _)| *ordinal);
        assert_eq!(ordinals, sorted);
        // The root's id is 0 with preorder 0; ids stay unique.
        assert_eq!(ordinals[0], (0, SourceAstId(0)));
        let unique: HashSet<_> = ordinals.iter().map(|(_, id)| *id).collect();
        assert_eq!(unique.len(), ordinals.len());
    }

    #[test]
    fn appending_a_member_does_not_renumber_existing_nodes() {
        let before = "module m; wire a; endmodule\n";
        let after = "module m; wire a; wire b; endmodule\n";
        let before_map = AstIdMap::from_source(&parse(before));
        let after_map = AstIdMap::from_source(&parse(after));
        let before_ids = collect_kind_ids(&before_map, &parse(before));
        let after_ids = collect_kind_ids(&after_map, &parse(after));

        for (kind, before_ids) in before_ids {
            let after_ids = &after_ids[&kind];
            assert_eq!(&after_ids[..before_ids.len()], before_ids.as_slice());
        }
        assert!(after_map.len() > before_map.len());
    }

    #[test]
    fn body_edits_keep_later_owner_ids() {
        let before = "module m; function void f(); endfunction module n; endmodule\n";
        let after = "module m; function void f(); wire x; endfunction module n; endmodule\n";
        let before_tree = parse(before);
        let after_tree = parse(after);
        let before_ids = collect_kind_ids(&AstIdMap::from_source(&before_tree), &before_tree);
        let after_ids = collect_kind_ids(&AstIdMap::from_source(&after_tree), &after_tree);
        assert_eq!(
            before_ids[&SyntaxKind::MODULE_DECLARATION],
            after_ids[&SyntaxKind::MODULE_DECLARATION]
        );
        assert_eq!(
            before_ids[&SyntaxKind::FUNCTION_DECLARATION],
            after_ids[&SyntaxKind::FUNCTION_DECLARATION]
        );
    }

    #[test]
    fn node_falls_back_to_stable_path_when_range_changes() {
        let before_tree = parse("module m; endmodule\n");
        let after_tree = parse("module m;");
        let map = AstIdMap::from_source(&before_tree);
        let module_id = collect_kind_ids(&map, &before_tree)[&SyntaxKind::MODULE_DECLARATION][0];

        let node = map.node(module_id, &after_tree).expect("stable path must resolve current AST");
        assert_eq!(node.kind(), SyntaxKind::MODULE_DECLARATION);
    }

    #[test]
    fn id_of_ptr_roundtrips() {
        let tree = parse("module m; endmodule\n");
        let map = AstIdMap::from_source(&tree);
        let root = tree.root();
        let id = map.id_of_node(root).expect("root must be numbered");
        assert_eq!(map.ptr(id), Some(SyntaxNodePtr::from_node(root)));
        assert_eq!(map.id_of_ptr(SyntaxNodePtr::from_node(root)), Some(id));
    }

    fn collect_kind_ids(
        map: &AstIdMap,
        tree: &SyntaxTree,
    ) -> HashMap<SyntaxKind, Vec<SourceAstId>> {
        let mut out = HashMap::new();
        for event in tree.root().node_preorder() {
            if let syntax::WalkEvent::Enter(node) = event
                && let Some(id) = map.id_of_node(node)
            {
                out.entry(node.kind()).or_insert_with(Vec::new).push(id);
            }
        }
        out
    }
}
