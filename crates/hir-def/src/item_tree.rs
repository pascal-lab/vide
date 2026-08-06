use std::hash::{Hash, Hasher};

use preproc_expand::file::HirFileId;
use rustc_hash::FxHasher;
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTokenWithParent, SyntaxTree, WalkEvent,
    ast::{self, AstNode},
    has_name::HasName,
    has_text_range::HasTextRange,
    ptr::SyntaxNodePtr,
};
use triomphe::Arc;
use utils::text_edit::TextRange;

use crate::{
    ast_id_map::{AstIdMap, SourceAstId},
    db::HirDefDb,
    owner::OwnerTable,
    source_projection::SourceOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemTreeId(u32);

impl ItemTreeId {
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub(crate) const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTreeItem {
    id: ItemTreeId,
    parent: Option<ItemTreeId>,
    kind: syntax::SyntaxKind,
    name: Option<SmolStr>,
    /// The item's node in the file's [`AstIdMap`]; `None` only for nodes
    /// without a root-buffer range (include buffers).
    ast_id: Option<SourceAstId>,
    header_fingerprint: u64,
}

impl ItemTreeItem {
    pub fn id(&self) -> ItemTreeId {
        self.id
    }

    pub fn parent(&self) -> Option<ItemTreeId> {
        self.parent
    }

    pub fn kind(&self) -> syntax::SyntaxKind {
        self.kind
    }

    pub fn name(&self) -> Option<&SmolStr> {
        self.name.as_ref()
    }

    /// Stable per-file id of the item's AST node.
    pub fn ast_id(&self) -> Option<SourceAstId> {
        self.ast_id
    }

    /// Fingerprint of the item kind, name, and header tokens.
    ///
    /// The body is intentionally not part of this value. This is the first
    /// incremental boundary for the future semantic identity layer.
    pub fn header_fingerprint(&self) -> u64 {
        self.header_fingerprint
    }
}

/// File-level structural summary. It intentionally contains no source ranges
/// or focus ranges; those belong to
/// [`crate::source_projection::SourceProjection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTree {
    file_id: HirFileId,
    owners: OwnerTable,
    items: Vec<ItemTreeItem>,
}

impl ItemTree {
    pub fn file_id(&self) -> HirFileId {
        self.file_id
    }

    pub fn owners(&self) -> &OwnerTable {
        &self.owners
    }

    pub fn root_owner(&self) -> Option<crate::owner::OwnerId> {
        self.owners.file_owner()
    }

    pub fn items(&self) -> impl Iterator<Item = &ItemTreeItem> {
        self.items.iter()
    }

    pub fn item(&self, id: ItemTreeId) -> Option<&ItemTreeItem> {
        self.items.get(id.raw() as usize)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn item_tree(db: &dyn HirDefDb, file_id: HirFileId, _key: ()) -> Arc<ItemTree> {
    let tree = db.parse(file_id);
    let ast_ids = db.ast_id_map(file_id);
    let source_text = file_id.as_file().map(|file_id| db.file_text(file_id));
    let owners = db.owner_table(file_id);
    Arc::new(build_item_tree(file_id, &tree, &ast_ids, source_text.as_deref(), (*owners).clone()))
}

pub(crate) fn set_item_tree_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    item_tree::set_lru_capacity(db, capacity);
}

pub(crate) fn build_source_projection(
    file_id: HirFileId,
    tree: &SyntaxTree,
) -> crate::source_projection::SourceProjection {
    let origins = member_nodes(tree)
        .into_iter()
        .map(|node| {
            let member = ast::Member::cast(node).expect("member node must cast");
            let full_range = node.text_range();
            let source_node = full_range.map(|_| SyntaxNodePtr::from_node(node));
            let (_, focus_range) = item_name(node);
            let focus_range = full_range.and(focus_range);
            Some(SourceOrigin::new(
                file_id,
                source_node,
                Some(member.syntax().kind()),
                full_range,
                focus_range,
            ))
        })
        .collect();
    crate::source_projection::SourceProjection::new(origins)
}

fn member_nodes<'a>(tree: &'a SyntaxTree) -> Vec<SyntaxNode<'a>> {
    tree.root()
        .into_iter()
        .flat_map(|root| {
            root.elem_preorder().filter_map(|event| match event {
                WalkEvent::Enter(SyntaxElement::Node(node))
                    if ast::Member::can_cast(node.kind()) =>
                {
                    Some(node)
                }
                _ => None,
            })
        })
        .collect()
}

fn build_item_tree(
    file_id: HirFileId,
    tree: &SyntaxTree,
    ast_ids: &AstIdMap,
    source_text: Option<&str>,
    owners: OwnerTable,
) -> ItemTree {
    let mut items = Vec::new();
    let mut parents = Vec::new();

    for node in member_nodes(tree) {
        let id = ItemTreeId::from_raw(items.len() as u32);
        let (name, _) = item_name(node);
        let header_range = item_header_range(node);
        let header_fingerprint = fingerprint(node.kind(), name.as_ref(), header_range, source_text);
        let full_range = node.text_range();
        let ast_id = full_range.and_then(|_| ast_ids.id_of_node(node));

        items.push(ItemTreeItem {
            id,
            parent: parents.last().copied(),
            kind: node.kind(),
            name,
            ast_id,
            header_fingerprint,
        });
        parents.push(id);
    }

    ItemTree { file_id, owners, items }
}

fn item_name(node: SyntaxNode<'_>) -> (Option<SmolStr>, Option<TextRange>) {
    let token = ast::ModuleDeclaration::cast(node)
        .and_then(|item| HasName::name(&item))
        .or_else(|| ast::FunctionDeclaration::cast(node).and_then(|item| HasName::name(&item)))
        .or_else(|| ast::ConfigDeclaration::cast(node).and_then(|item| item.name()))
        .or_else(|| ast::UdpDeclaration::cast(node).and_then(|item| item.name()))
        .or_else(|| ast::LibraryDeclaration::cast(node).and_then(|item| item.name()))
        .or_else(|| ast::GenerateBlock::cast(node).and_then(|item| HasName::name(&item)))
        .or_else(|| ast::ClassDeclaration::cast(node).and_then(|item| item.name()))
        .or_else(|| ast::CheckerDeclaration::cast(node).and_then(|item| item.name()))
        .or_else(|| ast::CovergroupDeclaration::cast(node).and_then(|item| item.name()))
        .or_else(|| ast::TypedefDeclaration::cast(node).and_then(|item| item.name()));

    if let Some(token) = token {
        return token_data(node, token);
    }

    // A few grammar nodes contain multiple declarations and therefore cannot
    // expose one canonical name yet. Keep the item in the tree and leave the
    // name unset instead of guessing from an arbitrary identifier token.
    (None, None)
}

fn token_data(
    node: SyntaxNode<'_>,
    token: SyntaxToken<'_>,
) -> (Option<SmolStr>, Option<TextRange>) {
    let range = SyntaxTokenWithParent { parent: node, tok: token }.text_range();
    (Some(token.value_text().to_smolstr()), range)
}

fn item_header_range(node: SyntaxNode<'_>) -> Option<TextRange> {
    ast::ModuleDeclaration::cast(node)
        .map(|item| item.header().syntax())
        .or_else(|| ast::FunctionDeclaration::cast(node).map(|item| item.prototype().syntax()))
        .and_then(|header| header.text_range())
}

fn fingerprint(
    kind: syntax::SyntaxKind,
    name: Option<&SmolStr>,
    header_range: Option<TextRange>,
    source_text: Option<&str>,
) -> u64 {
    let mut hasher = FxHasher::default();
    kind.hash(&mut hasher);
    name.hash(&mut hasher);

    if let (Some(range), Some(text)) = (header_range, source_text)
        && let Some(header) = text.get(usize::from(range.start())..usize::from(range.end()))
    {
        header.hash(&mut hasher);
    }

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use vfs::FileId;

    use super::*;
    use crate::source_projection::SourceProjection;

    fn parse(text: &str) -> SyntaxTree {
        SyntaxTree::from_text(text, "test.sv", "test.sv")
    }

    fn build(file_id: HirFileId, text: &str) -> ItemTree {
        let tree = parse(text);
        build_item_tree(
            file_id,
            &tree,
            &AstIdMap::from_source(&tree),
            Some(text),
            OwnerTable::default(),
        )
    }

    #[test]
    fn item_tree_tracks_nested_members_without_using_body_ranges_for_headers() {
        let before = "module top; function void f(); logic value; endfunction endmodule\n";
        let after =
            "module top; function void f(); logic value; value = 1; endfunction endmodule\n";
        let file_id = HirFileId::File(FileId::from_raw(0));
        let before = build(file_id, before);
        let after = build(file_id, after);

        let before_function = before
            .items()
            .find(|item| item.name().is_some_and(|name| name == "f"))
            .expect("function should be indexed");
        let after_function = after
            .items()
            .find(|item| item.name().is_some_and(|name| name == "f"))
            .expect("function should be indexed");
        assert_eq!(before, after);

        assert_eq!(before_function.header_fingerprint(), after_function.header_fingerprint());
        assert_eq!(before_function.parent(), after_function.parent());
    }

    #[test]
    fn source_projection_keeps_non_navigable_items_distinct_from_missing_items() {
        let file_id = HirFileId::File(FileId::from_raw(0));
        let projection =
            SourceProjection::new(vec![Some(SourceOrigin::new(file_id, None, None, None, None))]);

        assert_eq!(projection.len(), 1);
        assert!(!projection.origin(ItemTreeId::from_raw(0)).unwrap().is_navigable());
    }

    #[test]
    fn item_tree_items_carry_stable_ast_ids() {
        let text = "module top; function void f(); logic value; endfunction endmodule\n";
        let file_id = HirFileId::File(FileId::from_raw(0));
        let tree = parse(text);
        let ast_ids = AstIdMap::from_source(&tree);
        let item_tree =
            build_item_tree(file_id, &tree, &ast_ids, Some(text), OwnerTable::default());

        for item in item_tree.items() {
            let ast_id = item.ast_id().expect("root-buffer item must be numbered");
            let ptr = ast_ids.ptr(ast_id).expect("item ast id must resolve");
            assert_eq!(
                ptr.kind(),
                item.kind(),
                "item {} ast id must point back at the item node",
                item.name().map_or("<unnamed>", |name| name.as_str())
            );
        }
    }
}
