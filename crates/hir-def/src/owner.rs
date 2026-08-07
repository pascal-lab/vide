//! Unified owner identity.
//!
//! Every lowering container — file, module, generate block, procedural block,
//! subroutine — is one [`OwnerId`]. The id is interned by
//! `(file, kind, parent, slot)`, so it is independent of names, ranges, and
//! AST numbering. `slot` is the ordinal among the owner's direct owner
//! children; body syntax that is not itself an owner cannot change it.
//!
//! The per-file [`owner_table`] enumerates structural owners independently of
//! source projection. [`OwnerSourceMap`] joins those owners to the current
//! [`SourceAstId`](crate::ast_id_map::SourceAstId), keeping source identity and
//! owner identity separate.
//!
//! The id is `'static` via `unsafe(no_lifetime)` so it can be used as a Salsa
//! single query key. This is an explicit safety tradeoff: it is paired with
//! `revisions = usize::MAX`, and the owner identity contains only immutable
//! interned fields.

use base_db::salsa;
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SyntaxKind, SyntaxNode, SyntaxTree, WalkEvent,
    ast::{self, AstNode},
    has_name::HasName,
};
use triomphe::Arc;

use crate::{
    ast_id_map::{self, AstIdMap, SourceAstId, SyntaxFileId},
    block::BlockId,
    container::SubroutineScope,
    db::HirDefDb,
    module::{ModuleId, generate::generate_block_name},
};

/// The kind of container an owner represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OwnerKind {
    File,
    Module,
    GenerateBlock,
    ProceduralBlock,
    Block,
    Subroutine,
    Checker,
    Covergroup,
    ClockingBlock,
}
/// A unified owner identity: `Copy`, `'static`, and independent of source
/// ranges or names. `slot` is the ordinal among the owner's direct owner
/// children; body syntax that is not itself an owner cannot change it.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct OwnerId {
    #[returns(copy)]
    pub file: HirFileId,
    #[returns(copy)]
    pub kind: OwnerKind,
    #[returns(copy)]
    pub parent: Option<OwnerId>,
    #[returns(copy)]
    pub slot: u32,
}

impl PartialOrd for OwnerId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OwnerId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        salsa::plumbing::AsId::as_id(self).cmp(&salsa::plumbing::AsId::as_id(other))
    }
}

/// One entry of the per-file [`OwnerTable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerData {
    pub id: OwnerId,
    pub kind: OwnerKind,
    pub parent: Option<OwnerId>,
    pub name: SmolStr,
}

/// The canonical owner enumeration of a file, in source (DFS preorder) order.
///
/// This is structural data only. Source AST ids and ranges live in
/// [`OwnerSourceMap`] so body/source edits do not redefine owner identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OwnerTable {
    owners: Vec<OwnerData>,
}

impl OwnerTable {
    pub fn owners(&self) -> &[OwnerData] {
        &self.owners
    }

    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }

    /// The file owner (the first entry).
    pub fn file_owner(&self) -> Option<OwnerId> {
        self.owners.first().map(|owner| owner.id)
    }

    /// Owners of one kind, in source order.
    pub fn owners_of_kind(&self, kind: OwnerKind) -> impl Iterator<Item = &OwnerData> {
        self.owners.iter().filter(move |owner| owner.kind == kind)
    }
}

/// The source-side join between an [`OwnerId`] and the current AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerSourceMap {
    by_owner: FxHashMap<OwnerId, Option<SourceAstId>>,
    by_ast: FxHashMap<SourceAstId, OwnerId>,
}

impl OwnerSourceMap {
    pub fn source_ast_id(&self, owner: OwnerId) -> Option<SourceAstId> {
        self.by_owner.get(&owner).copied().flatten()
    }

    pub fn owner_by_ast(&self, ast_id: SourceAstId) -> Option<OwnerId> {
        self.by_ast.get(&ast_id).copied()
    }
}

#[salsa::tracked(lru = 256, returns(clone))]
pub(crate) fn owner_table(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<OwnerTable> {
    let file_id = file.hir_file(db);
    let tree = db.parse(file_id);
    Arc::new(owner_table_from_source(db, file_id, &tree))
}

#[salsa::tracked(lru = 256, returns(clone))]
pub(crate) fn owner_source_map(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<OwnerSourceMap> {
    let file_id = file.hir_file(db);
    let tree = db.parse(file_id);
    let ast_ids = ast_id_map::ast_id_map(db, file);
    Arc::new(owner_source_map_from_source(db, file_id, &tree, &ast_ids))
}

/// Owner-local source lookup. The single `OwnerId` key is intentional: body
/// queries can locate their current syntax without re-keying through a legacy
/// container id.
#[salsa::tracked(returns(copy))]
pub(crate) fn owner_source_ast_id(db: &dyn HirDefDb, owner: OwnerId) -> Option<SourceAstId> {
    db.owner_source_map(owner.file(db)).source_ast_id(owner)
}

pub(crate) fn set_owner_table_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    owner_table::set_lru_capacity(db, capacity);
    owner_source_map::set_lru_capacity(db, capacity);
}

fn owner_table_from_source(db: &dyn HirDefDb, file_id: HirFileId, tree: &SyntaxTree) -> OwnerTable {
    let owners = discover_owners(db, file_id, tree, None)
        .into_iter()
        .map(|owner| OwnerData {
            id: owner.id,
            kind: owner.kind,
            parent: owner.parent,
            name: owner.name,
        })
        .collect();
    OwnerTable { owners }
}

fn owner_source_map_from_source(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    tree: &SyntaxTree,
    ast_ids: &AstIdMap,
) -> OwnerSourceMap {
    let owners = discover_owners(db, file_id, tree, Some(ast_ids));
    let mut by_owner = FxHashMap::default();
    let mut by_ast = FxHashMap::default();
    for discovered in owners {
        by_owner.insert(discovered.id, discovered.ast_id);
        if let Some(ast_id) = discovered.ast_id {
            by_ast.insert(ast_id, discovered.id);
        }
    }
    OwnerSourceMap { by_owner, by_ast }
}

struct DiscoveredOwner {
    id: OwnerId,
    kind: OwnerKind,
    parent: Option<OwnerId>,
    name: SmolStr,
    ast_id: Option<SourceAstId>,
}

fn discover_owners(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    tree: &SyntaxTree,
    ast_ids: Option<&AstIdMap>,
) -> Vec<DiscoveredOwner> {
    let root_ast_id =
        tree.root().and_then(|root| ast_ids.and_then(|map| map.id_of_node_in_tree(tree, root)));
    let file_owner = OwnerId::new(db, file_id, OwnerKind::File, None, 0);
    let mut discovered = vec![DiscoveredOwner {
        id: file_owner,
        kind: OwnerKind::File,
        parent: None,
        name: SmolStr::new_static(""),
        ast_id: root_ast_id,
    }];

    let Some(root) = tree.root() else {
        return discovered;
    };

    let mut owners = vec![file_owner];
    let mut next_slots = vec![0u32];
    for event in root.node_preorder() {
        match event {
            WalkEvent::Enter(node) => {
                let Some(kind) = owner_kind_of(node) else {
                    continue;
                };
                let slot = next_slots.last_mut().expect("owner stack is non-empty");
                let owner_slot = *slot;
                *slot += 1;
                let parent = owners.last().copied();
                let owner = OwnerId::new(db, file_id, kind, parent, owner_slot);
                let name = owner_name(node);
                let ast_id = ast_ids.and_then(|map| map.id_of_node_in_tree(tree, node));
                discovered.push(DiscoveredOwner { id: owner, kind, parent, name, ast_id });
                owners.push(owner);
                next_slots.push(0);
            }
            WalkEvent::Leave(node) => {
                if owner_kind_of(node).is_some() {
                    owners.pop();
                    next_slots.pop();
                }
            }
        }
    }

    discovered
}


/// Syntax nodes that start a semantic owner. Generate branches deliberately
/// mirror `generate.rs`: a loop owns its block (the nested `GenerateBlock`
/// node is not a second owner), while an unwrapped branch member is promoted
/// to a generate-block owner.
fn owner_kind_of(node: SyntaxNode<'_>) -> Option<OwnerKind> {
    let kind = node.kind();
    match kind {
        SyntaxKind::MODULE_DECLARATION
        | SyntaxKind::INTERFACE_DECLARATION
        | SyntaxKind::PACKAGE_DECLARATION
        | SyntaxKind::PROGRAM_DECLARATION => Some(OwnerKind::Module),
        SyntaxKind::FUNCTION_DECLARATION | SyntaxKind::TASK_DECLARATION => {
            Some(OwnerKind::Subroutine)
        }
        SyntaxKind::GENERATE_BLOCK
            if node.parent().and_then(ast::LoopGenerate::cast).is_some() =>
        {
            None
        }
        SyntaxKind::GENERATE_BLOCK | SyntaxKind::LOOP_GENERATE => Some(OwnerKind::GenerateBlock),
        SyntaxKind::INITIAL_BLOCK
        | SyntaxKind::FINAL_BLOCK
        | SyntaxKind::ALWAYS_BLOCK
        | SyntaxKind::ALWAYS_COMB_BLOCK
        | SyntaxKind::ALWAYS_FF_BLOCK
        | SyntaxKind::ALWAYS_LATCH_BLOCK => Some(OwnerKind::ProceduralBlock),
        SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT | SyntaxKind::PARALLEL_BLOCK_STATEMENT => {
            Some(OwnerKind::Block)
        }
        SyntaxKind::CHECKER_DECLARATION => Some(OwnerKind::Checker),
        SyntaxKind::COVERGROUP_DECLARATION => Some(OwnerKind::Covergroup),
        SyntaxKind::CLOCKING_DECLARATION => Some(OwnerKind::ClockingBlock),
        _ if ast::Member::cast(node).is_some() && is_unwrapped_generate_branch(node) => {
            Some(OwnerKind::GenerateBlock)
        }
        _ => None,
    }
}

fn is_unwrapped_generate_branch(node: SyntaxNode<'_>) -> bool {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if ast::GenerateBlock::can_cast(ancestor.kind())
            || ast::LoopGenerate::can_cast(ancestor.kind())
            || ast::ModuleDeclaration::can_cast(ancestor.kind())
        {
            return false;
        }
        if ast::IfGenerate::can_cast(ancestor.kind())
            || ast::StandardCaseItem::can_cast(ancestor.kind())
            || ast::DefaultCaseItem::can_cast(ancestor.kind())
        {
            return true;
        }
        parent = ancestor.parent();
    }
    false
}

fn owner_name(node: SyntaxNode<'_>) -> SmolStr {
    let token = ast::ModuleDeclaration::cast(node)
        .and_then(|item| HasName::name(&item))
        .or_else(|| ast::FunctionDeclaration::cast(node).and_then(|item| HasName::name(&item)))
        .or_else(|| ast::GenerateBlock::cast(node).and_then(generate_block_name))
        .or_else(|| {
            ast::LoopGenerate::cast(node)
                .and_then(|loop_generate| loop_generate.block().as_generate_block())
                .and_then(generate_block_name)
        })
        .or_else(|| ast::BlockStatement::cast(node).and_then(|item| HasName::name(&item)));
    token.map(|token| token.value_text().to_smolstr()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Legacy id -> OwnerId
// ---------------------------------------------------------------------------

impl ModuleId {
    /// Canonical owner for a top-level module slot.
    pub fn owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        let table = db.owner_table(self.file_id);
        let file_owner = table.file_owner()?;
        let slot = u32::from(self.value.into_raw()) as usize;
        table
            .owners_of_kind(OwnerKind::Module)
            .filter(|owner| owner.parent == Some(file_owner))
            .nth(slot)
            .map(|owner| owner.id)
    }

    pub(crate) fn from_owner(db: &dyn HirDefDb, owner: OwnerId) -> Option<Self> {
        (owner.kind(db) == OwnerKind::Module).then_some(())?;
        let file_id = owner.file(db);
        let table = db.owner_table(file_id);
        let file_owner = table.file_owner()?;
        let slot = table
            .owners_of_kind(OwnerKind::Module)
            .filter(|candidate| candidate.parent == Some(file_owner))
            .position(|candidate| candidate.id == owner)?;
        let slot = u32::try_from(slot).ok()?;
        Some(Self::new(file_id, la_arena::Idx::from_raw(la_arena::RawIdx::from(slot))))
    }
}

impl BlockId {
    /// The unified owner of this block.
    pub fn owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        let src = self.loc().src;
        let ast_id = db.ast_id_map(src.file_id).id_of_ptr(src.value.node)?;
        db.owner_source_map(src.file_id).owner_by_ast(ast_id)
    }
}

impl crate::module::generate::GenerateBlockId {
    /// The unified owner of this generate block.
    pub fn owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        let src = self.loc().src;
        let ast_id = db.ast_id_map(src.file_id).id_of_ptr(src.value.node())?;
        db.owner_source_map(src.file_id).owner_by_ast(ast_id)
    }
}

impl SubroutineScope {
    /// The unified owner of this subroutine.
    pub fn owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        let src = crate::def_id::subroutine_src(db, self)?;
        let ast_id = db.ast_id_map(src.file_id).id_of_ptr(src.value.node)?;
        db.owner_source_map(src.file_id).owner_by_ast(ast_id)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        salsa::{self, Durability},
        source_db::{FileLoader, SourceDb, SourceFileKind, SourceRootDb},
        source_root::{SourceRoot, SourceRootId},
    };
    use preproc_expand::{db::PreprocDb, file::HirFileId};
    use rustc_hash::FxHashSet;
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use crate::{
        container::{SubroutineParent, SubroutineScope},
        db::HirDefDb,
        module::ModuleId,
    };

    const TOP: FileId = FileId::from_raw(0);
    const ROOT: SourceRootId = SourceRootId(0);
    const PROFILE: CompilationProfileId = CompilationProfileId(0);

    #[salsa::db]
    #[derive(Default)]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    #[salsa::db]
    impl salsa::Database for TestDb {}

    #[salsa::db]
    impl SourceDb for TestDb {}

    #[salsa::db]
    impl SourceRootDb for TestDb {}

    #[salsa::db]
    impl PreprocDb for TestDb {}

    #[salsa::db]
    impl HirDefDb for TestDb {}

    impl std::ops::Deref for TestDb {
        type Target = dyn HirDefDb;

        fn deref(&self) -> &Self::Target {
            self
        }
    }

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl FileLoader for TestDb {
        fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor);
            SourceRootDb::source_root(self, source_root_id).resolve_path(path)
        }
    }

    fn db_with_root_text(root_text: &str) -> TestDb {
        let top_path = abs_path("rtl/top.sv");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);
        let mut files = FxHashSet::default();
        files.insert(TOP);

        let project_config = ProjectConfig::new(
            vec![Some(PROFILE)],
            vec![CompilationProfile {
                source_roots: vec![ROOT],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig::default(),
            }],
        );

        let mut db = TestDb::default();
        db.set_files_with_durability(files, Durability::HIGH);
        db.set_project_config_with_durability(Arc::new(project_config), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        db.set_source_root_id_with_durability(TOP, ROOT, Durability::LOW);
        db.set_file_path_with_durability(TOP, Some(top_path), Durability::LOW);
        db.set_file_kind_with_durability(TOP, SourceFileKind::SystemVerilog, Durability::LOW);
        db.set_file_text_with_durability(TOP, Arc::from(root_text), Durability::LOW);
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    /// Structural fingerprint of an owner table: (kind, name, parent name).
    /// Comparable across databases, unlike the interned ids.
    fn fingerprint(table: &crate::owner::OwnerTable) -> Vec<(String, String, Option<String>)> {
        table
            .owners()
            .iter()
            .map(|owner| {
                let parent = owner.parent.and_then(|parent| {
                    table
                        .owners()
                        .iter()
                        .find(|candidate| candidate.id == parent)
                        .map(|candidate| candidate.name.to_string())
                });
                (format!("{:?}", owner.kind), owner.name.to_string(), parent)
            })
            .collect()
    }

    #[test]
    fn owner_table_enumerates_owners_with_parents_and_names() {
        let text = r#"
module m;
  task automatic t;
    begin : blk
      int x = 1;
    end
  endtask
  generate
    begin : g
      wire y;
    end
  endgenerate
endmodule
"#;
        let db = db_with_root_text(text);
        let table = db.owner_table(HirFileId::File(TOP));

        let rows: Vec<(String, String, Option<String>)> = table
            .owners()
            .iter()
            .map(|owner| {
                let parent = owner.parent.and_then(|parent| {
                    table
                        .owners()
                        .iter()
                        .find(|candidate| candidate.id == parent)
                        .map(|candidate| candidate.name.to_string())
                });
                (format!("{:?}", owner.kind), owner.name.to_string(), parent)
            })
            .collect();

        assert_eq!(
            rows,
            vec![
                ("File".to_owned(), "".to_owned(), None),
                ("Module".to_owned(), "m".to_owned(), Some("".to_owned())),
                ("Subroutine".to_owned(), "t".to_owned(), Some("m".to_owned())),
                ("Block".to_owned(), "blk".to_owned(), Some("t".to_owned())),
                ("GenerateBlock".to_owned(), "g".to_owned(), Some("m".to_owned())),
            ]
        );
    }

    #[test]
    fn owner_table_is_stable_across_body_edits() {
        let before = "module m; task automatic t; begin int x = 1; end endtask task automatic u; endtask endmodule\n";
        let after = "module m; task automatic t; begin int x = 1; x = x + 1; end endtask task automatic u; endtask endmodule\n";
        let mut db = db_with_root_text(before);
        let file_id = HirFileId::File(TOP);
        let before_table = db.owner_table(file_id);
        let before_ids: Vec<_> = before_table.owners().iter().map(|owner| owner.id).collect();

        db.set_file_text_with_durability(TOP, Arc::from(after), Durability::LOW);

        let after_table = db.owner_table(file_id);
        let after_ids: Vec<_> = after_table.owners().iter().map(|owner| owner.id).collect();
        assert_eq!(fingerprint(&before_table), fingerprint(&after_table));
        assert_eq!(before_ids, after_ids);
    }
    #[test]
    fn subroutine_body_relowers_after_body_edit() {
        let before = "module m; task automatic t; int x; endtask endmodule\n";
        let after = "module m; task automatic t; int x; int y; endtask endmodule\n";
        let mut db = db_with_root_text(before);
        let file_id = HirFileId::File(TOP);
        let owner = db
            .owner_table(file_id)
            .owners_of_kind(crate::owner::OwnerKind::Subroutine)
            .next()
            .expect("subroutine owner must exist")
            .id;

        assert_eq!(db.subroutine_body_with_source_map(owner).decls.len(), 1);

        db.set_file_text_with_durability(TOP, Arc::from(after), Durability::LOW);

        assert_eq!(db.subroutine_body_with_source_map(owner).decls.len(), 2);
    }

    #[test]
    fn owner_table_ids_survive_added_sibling() {
        let before = "module m; task automatic t; endtask endmodule\n";
        let after = "module m; task automatic t; endtask task automatic u; endtask endmodule\n";
        let before_table = db_with_root_text(before).owner_table(HirFileId::File(TOP));
        let after_table = db_with_root_text(after).owner_table(HirFileId::File(TOP));

        // The new sibling is appended last, so every existing owner keeps its
        // structural slot.
        assert_eq!(before_table.owners().len() + 1, after_table.owners().len());
        let before_rows: Vec<_> = fingerprint(&before_table);
        let after_rows: Vec<_> = fingerprint(&after_table);
        assert_eq!(before_rows, after_rows[..before_rows.len()]);
    }

    #[test]
    fn module_id_maps_to_its_owner() {
        let text = "module m; endmodule\nmodule n; endmodule\n";
        let db = db_with_root_text(text);
        let file_id = HirFileId::File(TOP);
        let hir_file = db.hir_file(file_id);
        let table = db.owner_table(file_id);

        for (local_module_id, module_info) in hir_file.modules.iter() {
            let module_id = ModuleId::new(file_id, local_module_id);
            let owner = module_id.owner(&db).expect("module must map to an owner");
            let expected = table
                .owners_of_kind(crate::owner::OwnerKind::Module)
                .find(|owner| owner.name == module_info.name.as_deref().unwrap_or(""))
                .expect("module owner must exist");
            assert_eq!(owner, expected.id);
            assert!(db.owner_source_ast_id(expected.id).is_some());
        }
    }

    #[test]
    fn block_id_maps_to_its_owner() {
        let text = "module m; initial begin : blk end endmodule\n";
        let db = db_with_root_text(text);
        let file_id = HirFileId::File(TOP);
        let table = db.owner_table(file_id);

        let block_owner = table
            .owners_of_kind(crate::owner::OwnerKind::Block)
            .next()
            .expect("block owner must exist");
        assert_eq!(block_owner.name, "blk");

        // Re-derive the block id the way lowering does and map it back.
        let module_id =
            ModuleId::new(file_id, db.hir_file(file_id).modules.iter().next().unwrap().0);
        let module = db.module_with_source_map(module_id);
        let block_id = module
            .stmts
            .iter()
            .filter_map(|(_, stmt)| match &stmt.kind {
                crate::stmt::StmtKind::Block(info)
                    if {
                        let node = info.block_id.loc().src.value.node;
                        let ast_id = db.owner_source_map(file_id).source_ast_id(block_owner.id);
                        Some(node) == ast_id.and_then(|id| db.ast_id_map(file_id).ptr(id))
                    } =>
                {
                    Some(info.block_id.clone())
                }
                _ => None,
            })
            .next()
            .expect("lowering must create the block");

        assert_eq!(block_id.owner(&db), Some(block_owner.id));
    }
    #[test]
    fn subroutine_scope_maps_to_its_owner() {
        let text = "module m; task automatic t; endtask endmodule\n";
        let db = db_with_root_text(text);
        let file_id = HirFileId::File(TOP);
        let table = db.owner_table(file_id);

        let subroutine_owner = table
            .owners_of_kind(crate::owner::OwnerKind::Subroutine)
            .next()
            .expect("subroutine owner must exist");
        assert_eq!(subroutine_owner.name, "t");

        let module_id =
            ModuleId::new(file_id, db.hir_file(file_id).modules.iter().next().unwrap().0);
        let module = db.module_with_source_map(module_id);
        let (value, _) = module
            .source_map()
            .subroutine_srcs
            .iter()
            .find(|(_, src)| {
                let ast_id = db.owner_source_map(file_id).source_ast_id(subroutine_owner.id);
                Some(src.node) == ast_id.and_then(|id| db.ast_id_map(file_id).ptr(id))
            })
            .expect("lowering must create the subroutine");
        let scope = SubroutineScope { cont_id: SubroutineParent::Module(module_id), value };

        assert_eq!(scope.owner(&db), Some(subroutine_owner.id));
    }

    #[test]
    fn subroutine_body_uses_owner_key() {
        let text = "module m; function void f(); logic x; endfunction endmodule\n";
        let db = db_with_root_text(text);
        let owner = db
            .owner_table(HirFileId::File(TOP))
            .owners_of_kind(crate::owner::OwnerKind::Subroutine)
            .next()
            .expect("subroutine owner must exist")
            .id;
        let ast_id = db.owner_source_ast_id(owner).expect("owner AST id must exist");
        let ptr = db.ast_id_map(HirFileId::File(TOP)).ptr(ast_id).expect("owner pointer");
        assert_eq!(ptr.kind(), syntax::SyntaxKind::FUNCTION_DECLARATION);
        let body = db.subroutine_body_with_source_map(owner);
        assert_eq!(body.decls.len(), 1);
    }
    #[test]
    fn subroutine_signature_is_owner_keyed() {
        let before = "module m; function void f(); logic x; endfunction endmodule\n";
        let after = "module m; function void f(); logic x; x = 1; endfunction endmodule\n";
        let mut db = db_with_root_text(before);
        let owner = db
            .owner_table(HirFileId::File(TOP))
            .owners_of_kind(crate::owner::OwnerKind::Subroutine)
            .next()
            .expect("subroutine owner must exist")
            .id;

        let before_signature = db.signature_for_owner(owner).expect("signature must exist");
        assert_eq!(before_signature.kind(), crate::item_tree::SignatureKind::Function);

        db.set_file_text_with_durability(TOP, Arc::from(after), Durability::LOW);

        assert_eq!(Some(before_signature), db.signature_for_owner(owner));
    }
    #[test]
    fn item_tree_excludes_source_ranges_from_semantic_data() {
        let before_text = "module m; function void f(); logic x; endfunction endmodule\n";
        let after_text = "module m; function void f(); logic x; x = 1; endfunction endmodule\n";
        let mut db = db_with_root_text(before_text);
        let file_id = HirFileId::File(TOP);

        let before_items = db.item_tree(file_id);
        let before_source = db.source_projection(file_id);
        assert_eq!(before_items.root_owner(), db.owner_table(file_id).file_owner());
        let function = before_items
            .items()
            .find(|item| item.kind() == syntax::SyntaxKind::FUNCTION_DECLARATION)
            .expect("function item must exist");
        let before_range =
            before_source.origin(function.id()).expect("function source must exist").full_range();

        db.set_file_text_with_durability(TOP, Arc::from(after_text), Durability::LOW);

        let after_items = db.item_tree(file_id);
        let after_source = db.source_projection(file_id);
        assert_eq!(before_items, after_items);
        let after_range =
            after_source.origin(function.id()).expect("function source must exist").full_range();
        assert_ne!(before_range, after_range);
    }
}
