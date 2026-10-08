//! Canonical semantic-owner identity.
//!
//! Every structural lowering owner is keyed by its immutable source anchor:
//! `(file, SourceAstId, kind)`. Parentage and names are revision data owned by
//! [`OwnerTable`]; they are deliberately not part of identity. This keeps an
//! owner stable when unrelated siblings or body-local syntax change while
//! still making source lookup a direct, allocation-free projection.
//!
//! A syntax node may start more than one owner (for example an unwrapped
//! generate branch containing a subroutine). `OwnerKind` distinguishes those
//! owners without introducing another ordinal identity system.
//!
//! The id is `'static` via `unsafe(no_lifetime)` so it can be used as a Salsa
//! query key. This is paired with `revisions = usize::MAX`; every interned
//! field is immutable across revisions.

use base_db::salsa;
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use smallvec::{SmallVec, smallvec};
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SyntaxElement, SyntaxKind, SyntaxNode, WalkEvent,
    ast::{self, AstNode},
    has_name::HasName,
};
use triomphe::Arc;

use crate::{
    ast_id_map::{SourceAstId, SyntaxFileId},
    db::HirDefDb,
    module::{ModuleKind, generate::generate_block_name},
};

/// The kind of container an owner represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OwnerKind {
    File,
    Module,
    AnonymousProgram,
    GenerateBlock,
    ProceduralBlock,
    Block,
    Subroutine,
    Checker,
    Covergroup,
    ClockingBlock,
}
/// Canonical semantic-owner identity.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct OwnerId {
    #[returns(copy)]
    pub file: HirFileId,
    #[returns(copy)]
    pub ast_id: SourceAstId,
    #[returns(copy)]
    pub kind: OwnerKind,
}

impl OwnerId {
    /// Lexical parent from the current structural owner graph.
    pub fn parent(self, db: &dyn HirDefDb) -> Option<Self> {
        db.owner_table(self.file(db)).owner(self).and_then(|owner| owner.parent)
    }

    /// Current name from the structural owner graph.
    pub fn name(self, db: &dyn HirDefDb) -> Option<SmolStr> {
        let table = db.owner_table(self.file(db));
        let name = table.owner(self)?.name.clone();
        (!name.is_empty()).then_some(name)
    }

    /// The language-level kind of a module-like owner.
    pub fn module_kind(self, db: &dyn HirDefDb) -> Option<ModuleKind> {
        db.owner_table(self.file(db)).owner(self)?.module_kind
    }
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OwnerData {
    pub id: OwnerId,
    pub source: SourceAstId,
    pub kind: OwnerKind,
    pub parent: Option<OwnerId>,
    pub name: SmolStr,
    pub module_kind: Option<ModuleKind>,
}

/// Source positions are not stored here. Every owner already carries its
/// stable [`SourceAstId`], while current ranges and pointers live in the AST
/// map/source projection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OwnerTable {
    owners: Vec<OwnerData>,
    by_id: FxHashMap<OwnerId, usize>,
    by_source: FxHashMap<(SourceAstId, OwnerKind), OwnerId>,
    by_name_kind: FxHashMap<(SmolStr, OwnerKind), SmallVec<[OwnerId; 1]>>,
}

impl OwnerTable {
    pub fn owners(&self) -> &[OwnerData] {
        &self.owners
    }

    pub fn owner(&self, owner: OwnerId) -> Option<&OwnerData> {
        self.by_id.get(&owner).map(|index| &self.owners[*index])
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

    pub fn owner_by_ast(&self, ast_id: SourceAstId, kind: OwnerKind) -> Option<OwnerId> {
        self.by_source.get(&(ast_id, kind)).copied()
    }

    /// Owners of this `(name, kind)`, in source order. Does not scan the table.
    pub fn owners_named(&self, name: &str, kind: OwnerKind) -> &[OwnerId] {
        self.by_name_kind.get(&(SmolStr::new(name), kind)).map(SmallVec::as_slice).unwrap_or(&[])
    }
}

pub(crate) struct OwnerTableBuilder<'db> {
    db: &'db dyn HirDefDb,
    file_id: HirFileId,
    table: OwnerTable,
    stack: Vec<OwnerId>,
}

impl<'db> OwnerTableBuilder<'db> {
    pub(crate) fn new(db: &'db dyn HirDefDb, file_id: HirFileId, root_ast_id: SourceAstId) -> Self {
        let file_owner = OwnerId::new(db, file_id, root_ast_id, OwnerKind::File);
        let mut table = OwnerTable::default();
        table.owners.push(OwnerData {
            id: file_owner,
            source: root_ast_id,
            kind: OwnerKind::File,
            parent: None,
            name: SmolStr::new_static(""),
            module_kind: None,
        });
        table.by_id.insert(file_owner, 0);
        table.by_source.insert((root_ast_id, OwnerKind::File), file_owner);
        Self { db, file_id, table, stack: vec![file_owner] }
    }

    fn kinds(&self, node: SyntaxNode<'_>) -> SmallVec<[OwnerKind; 2]> {
        owner_kinds_of(node)
    }

    pub(crate) fn enter(&mut self, node: SyntaxNode<'_>, ast_id: SourceAstId) {
        for kind in self.kinds(node) {
            let parent = self.stack.last().copied();
            let owner = OwnerId::new(self.db, self.file_id, ast_id, kind);
            let index = self.table.owners.len();
            let name = owner_name(node, kind);
            self.table.owners.push(OwnerData {
                id: owner,
                source: ast_id,
                kind,
                parent,
                name: name.clone(),
                module_kind: owner_module_kind(node, kind),
            });
            let replaced = self.table.by_id.insert(owner, index);
            debug_assert!(replaced.is_none(), "duplicate owner identity");
            let replaced = self.table.by_source.insert((ast_id, kind), owner);
            debug_assert!(replaced.is_none(), "duplicate owner source key");
            if !name.is_empty() {
                self.table.by_name_kind.entry((name, kind)).or_default().push(owner);
            }
            self.stack.push(owner);
        }
    }

    pub(crate) fn leave(&mut self, node: SyntaxNode<'_>) {
        for _ in self.kinds(node) {
            self.stack.pop().expect("owner stack always contains the file owner");
        }
    }

    pub(crate) fn finish(self) -> OwnerTable {
        debug_assert_eq!(self.stack.len(), 1);
        self.table
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn owner_table(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<OwnerTable> {
    let file_id = file.hir_file(db);
    let tree = db.parse(file_id);
    let ast_ids = crate::ast_id_map::ast_id_map(db, file);
    Arc::new(build_owner_table(db, file_id, &tree, &ast_ids))
}

pub(crate) fn build_owner_table(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    tree: &syntax::SyntaxTree,
    ast_ids: &crate::ast_id_map::AstIdMap,
) -> OwnerTable {
    let root = tree.root();
    assert!(
        matches!(root.kind(), SyntaxKind::COMPILATION_UNIT | SyntaxKind::LIBRARY_MAP),
        "owner table requires a compilation-unit or library-map syntax root"
    );
    let root_ast_id = ast_ids.id_of_node(root).unwrap_or(SourceAstId::from_raw(0));
    let mut builder = OwnerTableBuilder::new(db, file_id, root_ast_id);
    for event in root.elem_preorder() {
        match event {
            WalkEvent::Enter(SyntaxElement::Node(node)) => {
                let ast_id =
                    ast_ids.id_of_node(node).expect("every syntax node has a source identity");
                builder.enter(node, ast_id);
            }
            WalkEvent::Leave(SyntaxElement::Node(node)) => {
                builder.leave(node);
            }
            _ => {}
        }
    }
    builder.finish()
}

pub(crate) fn set_owner_table_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    owner_table::set_lru_capacity(db, capacity);
}

/// Syntax nodes that start semantic owners. Generate branches deliberately
/// mirror `generate.rs`: a loop owns its block (the nested `GenerateBlock`
/// node is not a second owner), while an unwrapped branch member is promoted
/// to a synthetic generate-block owner. If that member is itself an owner,
/// both owners share the same source AST id and are distinguished by kind.
pub(crate) fn owner_kinds_of(node: SyntaxNode<'_>) -> SmallVec<[OwnerKind; 2]> {
    let intrinsic = intrinsic_owner_kind(node);
    if ast::Member::cast(node).is_some()
        && is_unwrapped_generate_branch(node)
        && intrinsic != Some(OwnerKind::GenerateBlock)
    {
        let mut kinds = smallvec![OwnerKind::GenerateBlock];
        if let Some(intrinsic) = intrinsic {
            kinds.push(intrinsic);
        }
        return kinds;
    }

    intrinsic.into_iter().collect()
}

fn intrinsic_owner_kind(node: SyntaxNode<'_>) -> Option<OwnerKind> {
    match node.kind() {
        SyntaxKind::MODULE_DECLARATION
        | SyntaxKind::INTERFACE_DECLARATION
        | SyntaxKind::PACKAGE_DECLARATION
        | SyntaxKind::PROGRAM_DECLARATION => Some(OwnerKind::Module),
        SyntaxKind::ANONYMOUS_PROGRAM => Some(OwnerKind::AnonymousProgram),
        SyntaxKind::FUNCTION_DECLARATION | SyntaxKind::TASK_DECLARATION => {
            Some(OwnerKind::Subroutine)
        }
        SyntaxKind::GENERATE_BLOCK if node.parent().and_then(ast::LoopGenerate::cast).is_some() => {
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

fn owner_module_kind(node: SyntaxNode<'_>, kind: OwnerKind) -> Option<ModuleKind> {
    (kind == OwnerKind::Module)
        .then(|| ast::ModuleDeclaration::cast(node))
        .flatten()
        .map(ModuleKind::from_ast)
}

fn owner_name(node: SyntaxNode<'_>, kind: OwnerKind) -> SmolStr {
    let token = match kind {
        OwnerKind::Module => {
            ast::ModuleDeclaration::cast(node).and_then(|item| HasName::name(&item))
        }
        OwnerKind::Subroutine => {
            ast::FunctionDeclaration::cast(node).and_then(|item| HasName::name(&item))
        }
        OwnerKind::GenerateBlock => {
            ast::GenerateBlock::cast(node).and_then(generate_block_name).or_else(|| {
                ast::LoopGenerate::cast(node)
                    .and_then(|loop_generate| loop_generate.block().as_generate_block())
                    .and_then(generate_block_name)
            })
        }
        OwnerKind::Block => ast::BlockStatement::cast(node).and_then(|item| HasName::name(&item)),
        OwnerKind::Checker => ast::CheckerDeclaration::cast(node).and_then(|item| item.name()),
        OwnerKind::Covergroup => {
            ast::CovergroupDeclaration::cast(node).and_then(|item| item.name())
        }
        OwnerKind::ClockingBlock => {
            ast::ClockingDeclaration::cast(node).and_then(|item| item.block_name())
        }
        OwnerKind::File | OwnerKind::ProceduralBlock | OwnerKind::AnonymousProgram => None,
    };
    token.map(|token| token.value_text().to_smolstr()).unwrap_or_default()
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

    use super::OwnerKind;
    use crate::db::HirDefDb;

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
    impl crate::db::DesignGraphDb for TestDb {}

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
        db.set_file_kind_with_durability(TOP, SourceFileKind::SystemVerilog, Durability::LOW);
        db.set_file_text_with_durability(TOP, Arc::from(root_text), Durability::LOW);
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    #[test]
    fn a_class_is_a_syntax_record_not_an_owner() {
        let db = db_with_root_text(
            r#"
module m;
  class C extends Base;
    int value;
    function void tick();
    endfunction
  endclass
endmodule
"#,
        );
        let table = db.owner_table(HirFileId::File(TOP));
        assert!(
            table.owners().iter().all(|owner| owner.name.as_str() != "C"),
            "a class is not interned as an owner"
        );
        let module = *table.owners_named("m", OwnerKind::Module).first().expect("module owner");
        let body = db.body(module);
        let class = body.classes.values().next().expect("class syntax record");
        assert_eq!(class.name.as_deref(), Some("C"));
        assert_eq!(class.base_class_name.as_deref(), Some("Base"));
        assert_eq!(class.members.len(), 2);
        assert_eq!(class.members[0].kind, crate::aggregate::ClassMemberKind::Property);
        assert_eq!(class.members[1].kind, crate::aggregate::ClassMemberKind::Method);
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

        assert_eq!(db.body_with_source_map(owner).decls.len(), 1);

        db.set_file_text_with_durability(TOP, Arc::from(after), Durability::LOW);

        assert_eq!(db.body_with_source_map(owner).decls.len(), 2);
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
        let hir_file = db.body(db.owner_table(file_id).file_owner().expect("file owner"));
        let table = db.owner_table(file_id);

        for owner in hir_file.module_owners() {
            let module_info = db.body(owner);
            let expected = table
                .owners_of_kind(crate::owner::OwnerKind::Module)
                .find(|candidate| candidate.name == module_info.name.as_deref().unwrap_or(""))
                .expect("module owner must exist");
            assert_eq!(owner, expected.id);
        }
    }

    #[test]
    fn block_statements_store_their_canonical_owner() {
        let text = "module m; initial begin : blk end endmodule\n";
        let db = db_with_root_text(text);
        let file_id = HirFileId::File(TOP);
        let block_owner = db
            .owner_table(file_id)
            .owners_of_kind(crate::owner::OwnerKind::Block)
            .next()
            .expect("block owner must exist")
            .id;

        let module_id = db
            .body(db.owner_table(file_id).file_owner().expect("file owner"))
            .module_owners()
            .next()
            .expect("module owner");
        let module = db.body_with_source_map(module_id);
        let proc = module.procs.iter().next().expect("initial block must lower").1;
        let body = db.body_with_source_map(proc.owner);
        let lowered_owner = body
            .stmts
            .values()
            .find_map(|stmt| match stmt.kind {
                crate::stmt::StmtKind::Block(owner) => Some(owner),
                _ => None,
            })
            .expect("procedural body lowering must create the block");

        assert_eq!(lowered_owner, block_owner);
    }

    #[test]
    fn nested_blocks_share_one_body_and_keep_distinct_scopes() {
        let text = r#"
module m;
  initial begin : outer
    int x;
    begin : inner
      int y;
    end
  end
endmodule
"#;
        let db = db_with_root_text(text);
        let file_id = HirFileId::File(TOP);
        let module_id = db
            .body(db.owner_table(file_id).file_owner().expect("file owner"))
            .module_owners()
            .next()
            .expect("module owner");
        let module = db.body(module_id);
        let proc_owner = module.procs.values().next().expect("initial block must lower").owner;
        let body = db.body_with_source_map(proc_owner);
        let table = db.owner_table(file_id);
        let outer = table
            .owners_of_kind(OwnerKind::Block)
            .find(|owner| owner.name == "outer")
            .expect("outer block owner")
            .id;
        let inner = table
            .owners_of_kind(OwnerKind::Block)
            .find(|owner| owner.name == "inner")
            .expect("inner block owner")
            .id;

        assert_eq!(body.decls.len(), 2);
        assert_eq!(body.scope_graph.root(), Some(proc_owner));
        assert_eq!(body.scope(outer).and_then(|scope| scope.parent()), Some(proc_owner));
        assert_eq!(body.scope(inner).and_then(|scope| scope.parent()), Some(outer));
        assert_eq!(body.scope(outer).unwrap().declarators().len(), 1);
        assert_eq!(body.scope(inner).unwrap().declarators().len(), 1);

        let projected = db.body_with_source_map(inner);
        assert!(Arc::ptr_eq(&body, &projected), "block scopes must not open child body stores");
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

        let module_owner = db
            .body(db.owner_table(file_id).file_owner().expect("file owner"))
            .module_owners()
            .next()
            .expect("module owner");
        assert!(
            db.body(module_owner).subroutine_owners().any(|owner| owner == subroutine_owner.id),
            "module body must retain the canonical subroutine owner"
        );
        assert_eq!(db.subroutine(subroutine_owner.id).name.as_deref(), Some("t"));
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
        let ast_id = owner.ast_id(&db);
        let ptr = db.ast_id_map(HirFileId::File(TOP)).ptr(ast_id).expect("owner pointer");
        assert_eq!(ptr.kind(), syntax::SyntaxKind::FUNCTION_DECLARATION);
        let body = db.body_with_source_map(owner);
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
    fn body_edit_reuses_structural_owner_store() {
        let before = "module m; function void f(); logic x; x = 0; endfunction endmodule\n";
        let after = "module m; function void f(); logic x; x = 1; endfunction endmodule\n";
        let mut db = db_with_root_text(before);
        let file_id = HirFileId::File(TOP);
        let module_id = db
            .body(db.owner_table(file_id).file_owner().expect("file owner"))
            .module_owners()
            .next()
            .expect("module owner");
        let owner = db
            .owner_table(file_id)
            .owners_of_kind(OwnerKind::Subroutine)
            .next()
            .expect("subroutine owner must exist")
            .id;

        let item_tree_before = db.item_tree(file_id);
        let module_before = db.body_with_source_map(module_id);
        let body_before = db.body_with_source_map(owner);

        db.set_file_text_with_durability(TOP, Arc::from(after), Durability::LOW);

        let item_tree_after = db.item_tree(file_id);
        let module_after = db.body_with_source_map(module_id);
        let body_after = db.body_with_source_map(owner);

        assert!(Arc::ptr_eq(&item_tree_before, &item_tree_after));
        assert!(Arc::ptr_eq(&module_before, &module_after));
        assert!(!Arc::ptr_eq(&body_before, &body_after));
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
