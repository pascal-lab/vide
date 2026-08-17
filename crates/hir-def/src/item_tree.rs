use std::hash::{Hash, Hasher};

use preproc_expand::file::HirFileId;
use rustc_hash::{FxHashMap, FxHasher};
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTokenWithParent, SyntaxTree, TokenKind,
    WalkEvent,
    ast::{self, AstNode},
    has_name::HasName,
    has_text_range::HasTextRange,
    match_ast,
};
use triomphe::Arc;
use utils::text_edit::TextRange;

use crate::{
    ast_id_map::{self, AstIdMap, SourceAstId, SyntaxFileId},
    db::HirDefDb,
    owner::{OwnerId, OwnerTable},
    source_projection::SourceOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SignatureId(u32);

impl SignatureId {
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub(crate) const fn raw(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignatureKind {
    Task,
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignaturePortDirection {
    Input,
    Output,
    Inout,
    Ref,
    ConstRef,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignaturePort {
    direction: SignaturePortDirection,
    name: Option<SmolStr>,
    type_ast: Option<SourceAstId>,
}

impl SignaturePort {
    pub fn direction(&self) -> SignaturePortDirection {
        self.direction
    }

    pub fn name(&self) -> Option<&SmolStr> {
        self.name.as_ref()
    }

    pub fn type_ast(&self) -> Option<SourceAstId> {
        self.type_ast
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Signature {
    kind: SignatureKind,
    return_type_ast: Option<SourceAstId>,
    ports: Vec<SignaturePort>,
}

impl Signature {
    pub fn kind(&self) -> SignatureKind {
        self.kind
    }

    pub fn return_type_ast(&self) -> Option<SourceAstId> {
        self.return_type_ast
    }

    pub fn ports(&self) -> impl Iterator<Item = &SignaturePort> {
        self.ports.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemTreeItem {
    id: SourceAstId,
    parent: Option<SourceAstId>,
    kind: syntax::SyntaxKind,
    name: Option<SmolStr>,
    signature: Option<SignatureId>,
    header_fingerprint: u64,
}

impl ItemTreeItem {
    pub fn id(&self) -> SourceAstId {
        self.id
    }

    pub fn parent(&self) -> Option<SourceAstId> {
        self.parent
    }

    pub fn kind(&self) -> syntax::SyntaxKind {
        self.kind
    }

    pub fn name(&self) -> Option<&SmolStr> {
        self.name.as_ref()
    }

    /// Stable source identity of this item.
    pub fn ast_id(&self) -> SourceAstId {
        self.id
    }

    pub fn signature(&self) -> Option<SignatureId> {
        self.signature
    }

    /// Fingerprint of the item kind, name, and header tokens.
    ///
    /// The body is intentionally not part of this value. This is the first
    /// incremental boundary for the future semantic identity layer.
    pub fn header_fingerprint(&self) -> u64 {
        self.header_fingerprint
    }
}

/// A module declaration collected from the file-level structural summary.
///
/// It contains semantic header data and source identity, but no source range.
/// Ranges belong to [`crate::source_projection::SourceProjection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleHeader {
    owner: OwnerId,
    name: SmolStr,
    kind: crate::module::ModuleKind,
    source: SourceAstId,
}

impl ModuleHeader {
    pub fn owner(&self) -> OwnerId {
        self.owner
    }

    pub fn name(&self) -> &SmolStr {
        &self.name
    }

    pub fn kind(&self) -> crate::module::ModuleKind {
        self.kind
    }

    pub fn source(&self) -> SourceAstId {
        self.source
    }
}

/// File-level structural summary. It intentionally contains no source ranges
/// or focus ranges; those belong to
/// [`crate::source_projection::SourceProjection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTree {
    file_id: HirFileId,
    owners: Arc<OwnerTable>,
    items: Vec<ItemTreeItem>,
    by_id: FxHashMap<SourceAstId, usize>,
    signatures: Vec<Signature>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructureFingerprint(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationSkeleton {
    preprocessor_independent: bool,
    item_tree: Arc<ItemTree>,
}

impl DeclarationSkeleton {
    pub fn preprocessor_independent(&self) -> bool {
        self.preprocessor_independent
    }

    pub fn item_tree(&self) -> &ItemTree {
        &self.item_tree
    }

    pub fn matches(&self, authoritative: &ItemTree) -> bool {
        self.item_tree.structure_fingerprint() == authoritative.structure_fingerprint()
            && *self.item_tree == *authoritative
    }
}

impl ItemTree {
    pub fn structure_fingerprint(&self) -> StructureFingerprint {
        let mut hasher = FxHasher::default();
        self.file_id.hash(&mut hasher);
        self.owners.owners().hash(&mut hasher);
        self.items.hash(&mut hasher);
        self.signatures.hash(&mut hasher);
        StructureFingerprint(hasher.finish())
    }

    pub fn file_id(&self) -> HirFileId {
        self.file_id
    }

    pub fn root_owner(&self) -> Option<OwnerId> {
        self.owners.file_owner()
    }

    pub fn owners(&self) -> &OwnerTable {
        &self.owners
    }

    /// Module and package headers in source order.
    ///
    /// This is the file-level declaration seam. Consumers that only need
    /// headers must not enter a scope or body query to discover them.
    pub fn module_headers(&self) -> impl Iterator<Item = ModuleHeader> + '_ {
        self.owners.owners_of_kind(crate::owner::OwnerKind::Module).filter_map(|owner| {
            owner.module_kind.map(|kind| ModuleHeader {
                owner: owner.id,
                name: owner.name.clone(),
                kind,
                source: owner.source,
            })
        })
    }

    pub fn items(&self) -> impl Iterator<Item = &ItemTreeItem> {
        self.items.iter()
    }

    pub fn item(&self, id: SourceAstId) -> Option<&ItemTreeItem> {
        self.by_id.get(&id).map(|index| &self.items[*index])
    }

    pub fn signature(&self, id: SignatureId) -> Option<&Signature> {
        self.signatures.get(id.raw())
    }

    pub fn signatures(&self) -> impl Iterator<Item = (SignatureId, &Signature)> {
        self.signatures
            .iter()
            .enumerate()
            .map(|(raw, signature)| (SignatureId::from_raw(raw as u32), signature))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Extracts position-free structural data from the live parse. This query may
/// execute after any syntax change; equality backdating prevents body-only
/// changes from invalidating [`item_tree`] and its consumers.
#[salsa::tracked(lru = 128, returns(clone))]
fn item_tree_input(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<ItemTree> {
    let file_id = file.hir_file(db);
    let tree = db.parse(file_id);
    let ast_ids = ast_id_map::ast_id_map(db, file);
    let source_text = file_id.as_file().map(|file_id| db.file_text(file_id));
    let owners = db.owner_table(file_id);
    let (items, signatures) = build_item_tree_data(&tree, &ast_ids, source_text.as_deref());
    let by_id = items.iter().enumerate().map(|(index, item)| (item.id, index)).collect();
    Arc::new(ItemTree { file_id, owners, items, by_id, signatures })
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn item_tree(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<ItemTree> {
    item_tree_input(db, file)
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn declaration_skeleton(
    db: &dyn HirDefDb,
    file: SyntaxFileId,
) -> Option<Arc<DeclarationSkeleton>> {
    let file_id = file.hir_file(db);
    let HirFileId::File(source_file) = file_id else {
        return None;
    };
    let source_model = db.source_model(source_file);
    let tree = &source_model.syntax_tree;
    let ast_ids = AstIdMap::from_source(tree);
    let owners = Arc::new(crate::owner::build_owner_table(db, file_id, tree, &ast_ids));
    let source_text = db.file_text(source_file);
    let (items, signatures) = build_item_tree_data(tree, &ast_ids, Some(&source_text));
    let by_id = items.iter().enumerate().map(|(index, item)| (item.id, index)).collect();
    Some(Arc::new(DeclarationSkeleton {
        preprocessor_independent: source_model.preprocessor_independent,
        item_tree: Arc::new(ItemTree { file_id, owners, items, by_id, signatures }),
    }))
}

pub(crate) fn set_item_tree_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    item_tree_input::set_lru_capacity(db, capacity);
    item_tree::set_lru_capacity(db, capacity);
    declaration_skeleton::set_lru_capacity(db, capacity);
    item_for_owner::set_lru_capacity(db, capacity);
    signature_for_owner::set_lru_capacity(db, capacity);
}
#[salsa::tracked(lru = 256, returns(clone))]
pub(crate) fn item_for_owner(db: &dyn HirDefDb, owner: OwnerId) -> Option<ItemTreeItem> {
    db.item_tree(owner.file(db)).item(owner.ast_id(db)).cloned()
}

#[salsa::tracked(lru = 256, returns(clone))]
pub(crate) fn signature_for_owner(db: &dyn HirDefDb, owner: OwnerId) -> Option<Signature> {
    let item = item_for_owner(db, owner)?;
    let signature = item.signature?;
    db.item_tree(owner.file(db)).signature(signature).cloned()
}

pub(crate) fn build_source_projection(
    file_id: HirFileId,
    tree: &SyntaxTree,
    ast_ids: &AstIdMap,
) -> crate::source_projection::SourceProjection {
    let mut origins = rustc_hash::FxHashMap::default();
    let root = tree.root();
    for event in root.node_preorder() {
        let syntax::WalkEvent::Enter(node) = event else {
            continue;
        };
        let id =
            ast_ids.id_of_node_in_tree(tree, node).expect("every syntax node has an AST identity");
        let full_range = node.text_range();
        let source_node = ast_ids.ptr(id);
        let (_, focus_range) = item_name(node);
        let focus_range = full_range.and(focus_range);
        origins.insert(
            id,
            SourceOrigin::new(file_id, source_node, Some(node.kind()), full_range, focus_range),
        );
    }
    crate::source_projection::SourceProjection::new(origins)
}

fn is_body_boundary(node: SyntaxNode<'_>) -> bool {
    ast::FunctionDeclaration::can_cast(node.kind()) || ast::ProceduralBlock::can_cast(node.kind())
}

fn build_item_tree_data(
    tree: &SyntaxTree,
    ast_ids: &AstIdMap,
    source_text: Option<&str>,
) -> (Vec<ItemTreeItem>, Vec<Signature>) {
    let mut items = Vec::new();
    let mut signatures = Vec::new();
    let mut parents = Vec::new();
    let mut body_depth = 0usize;
    let root = tree.root();
    if root.kind() != syntax::SyntaxKind::COMPILATION_UNIT {
        // Library-map and other non-compilation-unit syntax roots have no
        // compilation-unit members, so they contribute no item-tree items.
        // Their declarations are lowered via `lower_library_map` instead.
        return (Vec::new(), Vec::new());
    }
    for event in root.elem_preorder() {
        match event {
            WalkEvent::Enter(SyntaxElement::Node(node)) => {
                let ast_id = ast_ids
                    .id_of_node_in_tree(tree, node)
                    .expect("every syntax node has an AST identity");

                if body_depth == 0 && ast::Member::can_cast(node.kind()) {
                    let (name, _) = item_name(node);
                    let header_range = item_header_range(node);
                    let header_fingerprint =
                        fingerprint(node.kind(), name.as_ref(), header_range, source_text);
                    let signature = ast::FunctionDeclaration::cast(node).map(|function| {
                        let id = SignatureId::from_raw(signatures.len() as u32);
                        signatures.push(lower_signature(function, ast_ids));
                        id
                    });

                    items.push(ItemTreeItem {
                        id: ast_id,
                        parent: parents.last().copied(),
                        kind: node.kind(),
                        name,
                        signature,
                        header_fingerprint,
                    });
                    parents.push(ast_id);
                }
                if is_body_boundary(node) {
                    body_depth += 1;
                }
            }
            WalkEvent::Leave(SyntaxElement::Node(node)) => {
                if is_body_boundary(node) {
                    body_depth -= 1;
                }
                if body_depth == 0 && ast::Member::can_cast(node.kind()) {
                    parents.pop().expect("item parent stack is balanced");
                }
            }
            _ => {}
        }
    }
    debug_assert!(parents.is_empty());
    (items, signatures)
}

#[cfg(test)]
fn build_item_tree(
    file_id: HirFileId,
    tree: &SyntaxTree,
    ast_ids: &AstIdMap,
    source_text: Option<&str>,
) -> ItemTree {
    let (items, signatures) = build_item_tree_data(tree, ast_ids, source_text);
    let by_id = items.iter().enumerate().map(|(index, item)| (item.id(), index)).collect();
    ItemTree { file_id, owners: Arc::new(OwnerTable::default()), items, by_id, signatures }
}
fn lower_signature(function: ast::FunctionDeclaration<'_>, ast_ids: &AstIdMap) -> Signature {
    let prototype = function.prototype();
    let kind = if function.as_task_declaration().is_some() {
        SignatureKind::Task
    } else {
        SignatureKind::Function
    };
    let return_type_ast = (kind == SignatureKind::Function)
        .then(|| ast_ids.id_of_node(prototype.return_type().syntax()))
        .flatten();
    let mut ports = Vec::new();

    if let Some(port_list) = prototype.port_list() {
        for port_base in port_list.ports().children() {
            if let Some(port) = port_base.as_function_port() {
                let mut direction =
                    signature_port_direction(port.direction().map(|token| token.kind()));
                if direction == SignaturePortDirection::Ref && port.const_keyword().is_some() {
                    direction = SignaturePortDirection::ConstRef;
                }
                ports.push(SignaturePort {
                    direction,
                    name: port.declarator().name().map(|token| token.value_text().to_smolstr()),
                    type_ast: port.data_type().and_then(|ty| ast_ids.id_of_node(ty.syntax())),
                });
            } else if port_base.as_default_function_port().is_some() {
                ports.push(SignaturePort {
                    direction: SignaturePortDirection::Input,
                    name: None,
                    type_ast: None,
                });
            }
        }
    }

    Signature { kind, return_type_ast, ports }
}

fn signature_port_direction(kind: Option<TokenKind>) -> SignaturePortDirection {
    match kind {
        Some(TokenKind::OUTPUT_KEYWORD) => SignaturePortDirection::Output,
        Some(TokenKind::IN_OUT_KEYWORD) => SignaturePortDirection::Inout,
        Some(TokenKind::REF_KEYWORD) => SignaturePortDirection::Ref,
        Some(TokenKind::INPUT_KEYWORD) | None => SignaturePortDirection::Input,
        Some(_) => SignaturePortDirection::Unknown,
    }
}

trait ItemName<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>>;
}

macro_rules! impl_has_name_item {
    ($($ty:ident),+ $(,)?) => {
        $(
            impl<'a> ItemName<'a> for ast::$ty<'a> {
                fn item_name(&self) -> Option<SyntaxToken<'a>> {
                    HasName::name(self)
                }
            }
        )+
    };
}

impl_has_name_item!(
    ModuleDeclaration,
    FunctionDeclaration,
    ConfigDeclaration,
    UdpDeclaration,
    LibraryDeclaration,
    GenerateBlock,
    BlockStatement,
    NonAnsiPort,
    PortReference,
    Declarator,
    IdentifierName,
    SpecparamDeclarator,
    ParamAssignment,
    PortConnection,
    HierarchicalInstance,
    Statement,
);

impl<'a> ItemName<'a> for ast::LoopGenerate<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>> {
        self.block().as_generate_block().and_then(|block| block.item_name())
    }
}

impl<'a> ItemName<'a> for ast::ImplicitAnsiPort<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>> {
        self.declarator().name()
    }
}

impl<'a> ItemName<'a> for ast::ExplicitAnsiPort<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>> {
        self.name()
    }
}

impl<'a> ItemName<'a> for ast::ModportItem<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>> {
        self.name()
    }
}

impl<'a> ItemName<'a> for ast::ClockingDeclaration<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>> {
        self.block_name()
    }
}

impl<'a> ItemName<'a> for ast::DefaultClockingReference<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>> {
        self.name()
    }
}

impl<'a> ItemName<'a> for ast::Coverpoint<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>> {
        self.label()?.name()
    }
}

impl<'a> ItemName<'a> for ast::CoverCross<'a> {
    fn item_name(&self) -> Option<SyntaxToken<'a>> {
        self.label()?.name()
    }
}

macro_rules! impl_direct_item_name {
    ($($ty:ident),+ $(,)?) => {
        $(
            impl<'a> ItemName<'a> for ast::$ty<'a> {
                fn item_name(&self) -> Option<SyntaxToken<'a>> {
                    self.name()
                }
            }
        )+
    };
}

impl_direct_item_name!(
    ClassDeclaration,
    CheckerDeclaration,
    CovergroupDeclaration,
    TypedefDeclaration,
);

fn item_name(node: SyntaxNode<'_>) -> (Option<SmolStr>, Option<TextRange>) {
    let token = match_ast! { node,
        ast::ModuleDeclaration[item] => item.item_name(),
        ast::FunctionDeclaration[item] => item.item_name(),
        ast::ConfigDeclaration[item] => item.item_name(),
        ast::UdpDeclaration[item] => item.item_name(),
        ast::LibraryDeclaration[item] => item.item_name(),
        ast::GenerateBlock[item] => item.item_name(),
        ast::LoopGenerate[item] => item.item_name(),
        ast::BlockStatement[item] => item.item_name(),
        ast::NonAnsiPort[item] => item.item_name(),
        ast::PortReference[item] => item.item_name(),
        ast::Declarator[item] => item.item_name(),
        ast::IdentifierName[item] => item.item_name(),
        ast::SpecparamDeclarator[item] => item.item_name(),
        ast::ParamAssignment[item] => item.item_name(),
        ast::PortConnection[item] => item.item_name(),
        ast::HierarchicalInstance[item] => item.item_name(),
        ast::Statement[item] => item.item_name(),
        ast::ImplicitAnsiPort[item] => item.item_name(),
        ast::ExplicitAnsiPort[item] => item.item_name(),
        ast::ModportItem[item] => item.item_name(),
        ast::ClockingDeclaration[item] => item.item_name(),
        ast::DefaultClockingReference[item] => item.item_name(),
        ast::Coverpoint[item] => item.item_name(),
        ast::CoverCross[item] => item.item_name(),
        ast::ClassDeclaration[item] => item.item_name(),
        ast::CheckerDeclaration[item] => item.item_name(),
        ast::CovergroupDeclaration[item] => item.item_name(),
        ast::TypedefDeclaration[item] => item.item_name(),
        _ => None,
    };

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
    use rustc_hash::FxHashMap;
    use vfs::FileId;

    use super::*;
    use crate::source_projection::SourceProjection;

    fn parse(text: &str) -> SyntaxTree {
        SyntaxTree::from_file_in_memory(text, "test.sv", "test.sv")
    }

    fn build(file_id: HirFileId, text: &str) -> ItemTree {
        let tree = parse(text);
        build_item_tree(file_id, &tree, &AstIdMap::from_source(&tree), Some(text))
    }

    #[test]
    fn item_tree_excludes_body_local_members() {
        let before = "module top; function void f(); logic value; endfunction endmodule\n";
        let after = "module top; function void f(); logic value; logic added; value = 1; endfunction endmodule\n";
        let file_id = HirFileId::File(FileId::from_raw(0));
        let before = build(file_id, before);
        let after = build(file_id, after);
        assert_eq!(before.len(), 2, "only the module and function are structural items");

        let before_function = before
            .items()
            .find(|item| item.name().is_some_and(|name| name == "f"))
            .expect("function should be indexed");
        let after_function = after
            .items()
            .find(|item| item.name().is_some_and(|name| name == "f"))
            .expect("function should be indexed");
        let module = before
            .items()
            .find(|item| item.kind() == syntax::SyntaxKind::MODULE_DECLARATION)
            .expect("module should be indexed");
        assert_eq!(before_function.parent(), Some(module.id()));
        assert_eq!(before, after);
        let signature = before
            .signature(before_function.signature().expect("function signature"))
            .expect("function signature must exist");
        assert_eq!(signature.kind(), SignatureKind::Function);
        assert!(signature.return_type_ast().is_some());

        assert_eq!(before_function.header_fingerprint(), after_function.header_fingerprint());
        assert_eq!(before_function.parent(), after_function.parent());
    }

    #[test]
    fn item_tree_builds_empty_for_library_map() {
        let text = "library foo \"dir/*.sv\";\n";
        let file_id = HirFileId::File(FileId::from_raw(0));
        let tree = SyntaxTree::from_library_map_text(text, "test.map", "test.map");
        let ast_ids = AstIdMap::from_source(&tree);
        let item_tree = build_item_tree(file_id, &tree, &ast_ids, Some(text));
        assert_eq!(item_tree.len(), 0, "library-map files contribute no item-tree items");
    }

    #[test]
    fn source_projection_keeps_non_navigable_items_distinct_from_missing_items() {
        let file_id = HirFileId::File(FileId::from_raw(0));
        let item_id = SourceAstId::from_raw(1);
        let mut origins = FxHashMap::default();
        origins.insert(item_id, SourceOrigin::new(file_id, None, None, None, None));
        let projection = SourceProjection::new(origins);

        assert_eq!(projection.len(), 1);
        assert!(!projection.origin(item_id).unwrap().is_navigable());
    }

    #[test]
    fn item_tree_items_carry_stable_ast_ids() {
        let text = "module top; function void f(); logic value; endfunction endmodule\n";
        let file_id = HirFileId::File(FileId::from_raw(0));
        let tree = parse(text);
        let ast_ids = AstIdMap::from_source(&tree);
        let item_tree = build_item_tree(file_id, &tree, &ast_ids, Some(text));

        for item in item_tree.items() {
            let ast_id = item.ast_id();
            let ptr = ast_ids.ptr(ast_id).expect("item ast id must resolve");
            assert_eq!(
                ptr.kind(),
                item.kind(),
                "item {} ast id must point back at the item node",
                item.name().map_or("<unnamed>", |name| name.as_str())
            );
        }
    }

    #[test]
    fn source_projection_focuses_named_ast_nodes() {
        let text = "module top; function void f(); logic value; endfunction generate for (genvar i = 0; i < 1; i++) begin : generated end endgenerate endmodule\n";
        let file_id = HirFileId::File(FileId::from_raw(0));
        let tree = parse(text);
        let ast_ids = AstIdMap::from_source(&tree);
        let projection = build_source_projection(file_id, &tree, &ast_ids);

        let mut names = FxHashMap::default();
        let root = tree.root();
        for event in root.node_preorder() {
            let syntax::WalkEvent::Enter(node) = event else {
                continue;
            };
            let Some(id) = ast_ids.id_of_node_in_tree(&tree, node) else {
                continue;
            };
            let Some(origin) = projection.origin(id) else {
                continue;
            };
            let Some(focus) = origin.focus_range() else {
                continue;
            };
            names.insert(node.kind(), &text[usize::from(focus.start())..usize::from(focus.end())]);
        }

        assert_eq!(names.get(&syntax::SyntaxKind::MODULE_DECLARATION), Some(&"top"));
        assert_eq!(names.get(&syntax::SyntaxKind::FUNCTION_DECLARATION), Some(&"f"));
        assert_eq!(names.get(&syntax::SyntaxKind::DECLARATOR), Some(&"value"));
        assert_eq!(names.get(&syntax::SyntaxKind::LOOP_GENERATE), Some(&"generated"));
    }
}
