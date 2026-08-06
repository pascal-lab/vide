//! Unified owner identity.
//!
//! Every lowering container — file, module, generate block, procedural block,
//! subroutine — is one [`OwnerId`]. The id is interned by
//! `(file, kind, parent, name, ast_id)` so it is `Copy` and equality is
//! identity, and it is `'static` (via `unsafe(no_lifetime)`) so it can be
//! stored across salsa revisions.
//!
//! The per-file [`owner_table`] enumerates owners from the syntax tree in
//! source order, independently of any lowering. It is the incrementality
//! foundation of the rearchitecture: a body edit changes the owner *contents*
//! but never the owner *set* (same nodes, same ids), so the table — and
//! everything keyed by [`OwnerId`] — survives a body edit without recompute.
//! The design doc sketches a salsa *tracked* struct for this; crates.io salsa
//! 0.28.2 tracked structs cannot erase the `'db` lifetime, so the id is
//! interned instead (the [`DefOrigin`](crate::symbol::DefOrigin) pattern),
//! which also satisfies the salsa single-key rule for the queries that will
//! consume it.

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
use utils::get::Get;

use crate::{
    ast_id_map::{AstIdMap, SourceAstId},
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
    Block,
    Subroutine,
    /// Not enumerated by [`owner_table`] yet; reserved for the per-owner
    /// queries that follow the container split.
    Checker,
    Covergroup,
    ClockingBlock,
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
struct InternedOwnerId {
    #[returns(copy)]
    file: HirFileId,
    #[returns(copy)]
    kind: OwnerKind,
    #[returns(copy)]
    parent: Option<OwnerId>,
    name: SmolStr,
    #[returns(copy)]
    ast_id: Option<SourceAstId>,
}

/// A unified owner identity: `Copy`, `'static`, path-shaped (parent chain).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnerId(InternedOwnerId);

impl PartialOrd for OwnerId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OwnerId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        salsa::plumbing::AsId::as_id(&self.0).cmp(&salsa::plumbing::AsId::as_id(&other.0))
    }
}

impl OwnerId {
    pub fn new(
        db: &dyn HirDefDb,
        file: HirFileId,
        kind: OwnerKind,
        parent: Option<OwnerId>,
        name: SmolStr,
        ast_id: Option<SourceAstId>,
    ) -> Self {
        Self(InternedOwnerId::new(db, file, kind, parent, name, ast_id))
    }

    pub fn file(self, db: &dyn HirDefDb) -> HirFileId {
        self.0.file(db)
    }

    pub fn kind(self, db: &dyn HirDefDb) -> OwnerKind {
        self.0.kind(db)
    }

    pub fn parent(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        self.0.parent(db)
    }

    pub fn name(self, db: &dyn HirDefDb) -> &SmolStr {
        self.0.name(db)
    }

    pub fn ast_id(self, db: &dyn HirDefDb) -> Option<SourceAstId> {
        self.0.ast_id(db)
    }
}

/// One entry of the per-file [`OwnerTable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerData {
    pub id: OwnerId,
    pub kind: OwnerKind,
    pub parent: Option<OwnerId>,
    pub name: SmolStr,
    pub ast_id: Option<SourceAstId>,
}

/// The canonical owner enumeration of a file, in source (DFS preorder) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerTable {
    owners: Vec<OwnerData>,
    by_ast: FxHashMap<SourceAstId, OwnerId>,
}

impl OwnerTable {
    pub fn owners(&self) -> &[OwnerData] {
        &self.owners
    }

    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }

    /// The owner whose AST node carries `ast_id`, when there is one.
    pub fn owner_by_ast(&self, ast_id: SourceAstId) -> Option<OwnerId> {
        self.by_ast.get(&ast_id).copied()
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

#[salsa::tracked(lru = 256, returns(clone))]
pub(crate) fn owner_table(db: &dyn HirDefDb, file_id: HirFileId, _key: ()) -> Arc<OwnerTable> {
    let tree = db.parse(file_id);
    let ast_ids = db.ast_id_map(file_id);
    Arc::new(owner_table_from_source(db, file_id, &tree, &ast_ids))
}

pub(crate) fn set_owner_table_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    owner_table::set_lru_capacity(db, capacity);
}

fn owner_table_from_source(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    tree: &SyntaxTree,
    ast_ids: &AstIdMap,
) -> OwnerTable {
    let mut owners = Vec::new();
    let mut by_ast = FxHashMap::default();

    // The file itself is the root owner; every top-level owner is its child.
    let root_ast_id = tree.root().and_then(|root| ast_ids.id_of_node(root));
    let file_owner =
        OwnerId::new(db, file_id, OwnerKind::File, None, SmolStr::new_static(""), root_ast_id);
    owners.push(OwnerData {
        id: file_owner,
        kind: OwnerKind::File,
        parent: None,
        name: SmolStr::new_static(""),
        ast_id: root_ast_id,
    });
    if let Some(ast_id) = root_ast_id {
        by_ast.insert(ast_id, file_owner);
    }

    let Some(root) = tree.root() else {
        return OwnerTable { owners, by_ast };
    };

    let mut stack = vec![file_owner];
    for event in root.node_preorder() {
        match event {
            WalkEvent::Enter(node) => {
                let Some(kind) = owner_kind_of(node.kind()) else {
                    continue;
                };
                let name = owner_name(node);
                let ast_id = ast_ids.id_of_node(node);
                let owner =
                    OwnerId::new(db, file_id, kind, stack.last().copied(), name.clone(), ast_id);
                owners.push(OwnerData {
                    id: owner,
                    kind,
                    parent: stack.last().copied(),
                    name,
                    ast_id,
                });
                if let Some(ast_id) = ast_id {
                    by_ast.insert(ast_id, owner);
                }
                stack.push(owner);
            }
            WalkEvent::Leave(node) => {
                if owner_kind_of(node.kind()).is_some() {
                    stack.pop();
                }
            }
        }
    }

    OwnerTable { owners, by_ast }
}

/// The node kinds that start a new owner; everything else belongs to the
/// enclosing owner's body.
fn owner_kind_of(kind: SyntaxKind) -> Option<OwnerKind> {
    match kind {
        SyntaxKind::MODULE_DECLARATION
        | SyntaxKind::INTERFACE_DECLARATION
        | SyntaxKind::PACKAGE_DECLARATION
        | SyntaxKind::PROGRAM_DECLARATION => Some(OwnerKind::Module),
        SyntaxKind::FUNCTION_DECLARATION | SyntaxKind::TASK_DECLARATION => {
            Some(OwnerKind::Subroutine)
        }
        SyntaxKind::GENERATE_BLOCK | SyntaxKind::LOOP_GENERATE => Some(OwnerKind::GenerateBlock),
        SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT | SyntaxKind::PARALLEL_BLOCK_STATEMENT => {
            Some(OwnerKind::Block)
        }
        _ => None,
    }
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
    /// The unified owner of this module, when its node is in the file's AST
    /// id map (root-buffer modules always are).
    pub fn owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        let lowered_file = db.hir_file_with_source_map(self.file_id);
        let src = lowered_file.source_map().get(self.value)?;
        let ast_id = db.ast_id_map(self.file_id).id_of_ptr(src.node)?;
        db.owner_table(self.file_id).owner_by_ast(ast_id)
    }
}

impl BlockId {
    /// The unified owner of this block.
    pub fn owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        let src = self.loc().src;
        let ast_id = db.ast_id_map(src.file_id).id_of_ptr(src.value.node)?;
        db.owner_table(src.file_id).owner_by_ast(ast_id)
    }
}

impl crate::module::generate::GenerateBlockId {
    /// The unified owner of this generate block.
    pub fn owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        let src = self.loc().src;
        let ast_id = db.ast_id_map(src.file_id).id_of_ptr(src.value.node())?;
        db.owner_table(src.file_id).owner_by_ast(ast_id)
    }
}

impl SubroutineScope {
    /// The unified owner of this subroutine.
    pub fn owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        let src = crate::def_id::subroutine_src(db, self)?;
        let ast_id = db.ast_id_map(src.file_id).id_of_ptr(src.value.node)?;
        db.owner_table(src.file_id).owner_by_ast(ast_id)
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
        ast_id_map::SourceAstId,
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

    /// Structural fingerprint of an owner table: (kind, name, parent name,
    /// ast id). Comparable across databases, unlike the interned ids.
    fn fingerprint(
        table: &crate::owner::OwnerTable,
    ) -> Vec<(String, String, Option<String>, Option<SourceAstId>)> {
        table
            .owners()
            .iter()
            .map(|owner| {
                let parent = owner.parent.map(|parent| {
                    table
                        .owners()
                        .iter()
                        .find(|candidate| candidate.id == parent)
                        .map(|candidate| candidate.name.to_string())
                        .unwrap_or_default()
                });
                (format!("{:?}", owner.kind), owner.name.to_string(), parent, owner.ast_id)
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
                let parent = owner.parent.map(|parent| parent.name(&db).to_string());
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
        let before = "module m; task automatic t; begin int x = 1; end endtask endmodule\n";
        let after =
            "module m; task automatic t; begin int x = 1; x = x + 1; end endtask endmodule\n";
        let before_table = db_with_root_text(before).owner_table(HirFileId::File(TOP));
        let after_table = db_with_root_text(after).owner_table(HirFileId::File(TOP));

        assert_eq!(fingerprint(&before_table), fingerprint(&after_table));
        assert_eq!(before_table.owners().len(), after_table.owners().len());
    }

    #[test]
    fn owner_table_ids_survive_added_sibling() {
        let before = "module m; task automatic t; endtask endmodule\n";
        let after = "module m; task automatic t; endtask task automatic u; endtask endmodule\n";
        let before_table = db_with_root_text(before).owner_table(HirFileId::File(TOP));
        let after_table = db_with_root_text(after).owner_table(HirFileId::File(TOP));

        // The new sibling is appended last, so every existing owner keeps its
        // source position and its ast id.
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
            assert!(owner.ast_id(&db).is_some());
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
                        Some(node)
                            == block_owner.ast_id.and_then(|id| db.ast_id_map(file_id).ptr(id))
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
                Some(src.node)
                    == subroutine_owner.ast_id.and_then(|id| db.ast_id_map(file_id).ptr(id))
            })
            .expect("lowering must create the subroutine");
        let scope = SubroutineScope { cont_id: SubroutineParent::Module(module_id), value };

        assert_eq!(scope.owner(&db), Some(subroutine_owner.id));
    }
}
