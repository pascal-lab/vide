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

use crate::{db::HirDefDb, source_projection::SourceOrigin};

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

    /// Fingerprint of the item kind, name, and header tokens.
    ///
    /// The body is intentionally not part of this value. This is the first
    /// incremental boundary for the future semantic identity layer.
    pub fn header_fingerprint(&self) -> u64 {
        self.header_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTree {
    file_id: HirFileId,
    items: Vec<ItemTreeItem>,
}

impl ItemTree {
    pub fn file_id(&self) -> HirFileId {
        self.file_id
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemTreeData {
    tree: ItemTree,
    pub(crate) source_projection: crate::source_projection::SourceProjection,
}
#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn item_tree_data(db: &dyn HirDefDb, file_id: HirFileId, _key: ()) -> Arc<ItemTreeData> {
    let tree = db.parse(file_id);
    let source_text = file_id.as_file().map(|file_id| db.file_text(file_id));
    Arc::new(build_item_tree(file_id, &tree, source_text.as_deref()))
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn item_tree(db: &dyn HirDefDb, file_id: HirFileId, _key: ()) -> Arc<ItemTree> {
    Arc::new(item_tree_data(db, file_id, ()).tree.clone())
}

pub(crate) fn set_item_tree_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    item_tree_data::set_lru_capacity(db, capacity);
    item_tree::set_lru_capacity(db, capacity);
}

fn build_item_tree(
    file_id: HirFileId,
    tree: &SyntaxTree,
    source_text: Option<&str>,
) -> ItemTreeData {
    let mut items = Vec::new();
    let mut origins = Vec::new();
    let mut parents = Vec::new();

    if let Some(root) = tree.root() {
        for event in root.elem_preorder() {
            match event {
                WalkEvent::Enter(SyntaxElement::Node(node))
                    if ast::Member::can_cast(node.kind()) =>
                {
                    let id = ItemTreeId::from_raw(items.len() as u32);
                    let member = ast::Member::cast(node)
                        .expect("Member::can_cast must produce a member node");
                    let parent = parents.last().copied();
                    let (name, focus_range) = item_name(node);
                    let header_range = item_header_range(node);
                    let header_fingerprint =
                        fingerprint(node.kind(), name.as_ref(), header_range, source_text);
                    let full_range = node.text_range();
                    let source_node = full_range.map(|_| SyntaxNodePtr::from_node(node));
                    let focus_range = full_range.and(focus_range);

                    items.push(ItemTreeItem {
                        id,
                        parent,
                        kind: node.kind(),
                        name,
                        header_fingerprint,
                    });
                    origins.push(Some(SourceOrigin::new(
                        file_id,
                        source_node,
                        Some(member.syntax().kind()),
                        full_range,
                        focus_range,
                    )));
                    parents.push(id);
                }
                WalkEvent::Leave(SyntaxElement::Node(node))
                    if ast::Member::can_cast(node.kind()) =>
                {
                    let popped = parents.pop();
                    debug_assert!(popped.is_some());
                }
                _ => {}
            }
        }
    }

    ItemTreeData {
        tree: ItemTree { file_id, items },
        source_projection: crate::source_projection::SourceProjection::new(origins),
    }
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

    #[test]
    fn item_tree_tracks_nested_members_without_using_body_ranges_for_headers() {
        let before = "module top; function void f(); logic value; endfunction endmodule\n";
        let after =
            "module top; function void f(); logic value; value = 1; endfunction endmodule\n";
        let file_id = HirFileId::File(FileId::from_raw(0));
        let before = build_item_tree(file_id, &parse(before), Some(before));
        let after = build_item_tree(file_id, &parse(after), Some(after));

        let before_function = before
            .tree
            .items()
            .find(|item| item.name().is_some_and(|name| name == "f"))
            .expect("function should be indexed");
        let after_function = after
            .tree
            .items()
            .find(|item| item.name().is_some_and(|name| name == "f"))
            .expect("function should be indexed");

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
}
