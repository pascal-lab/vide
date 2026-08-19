//! Locate compilation-unit owners.
//!
//! L0 [`UnitCatalog`] is a name → file locator, not identity. Identity is the
//! paid-parse [`OwnerId`] (`SourceAstId` / `HirFileId::Macro`). Production
//! resolution must not project `UnitId` → `OwnerId` by name.

use std::cell::Cell;

use design_graph::UnitKind;
use preproc_expand::{file::HirFileId, macro_file::macro_files_for_file};
use rustc_hash::FxHashSet;
use vfs::FileId;

use crate::{
    db::HirDefDb,
    module::ModuleKind,
    owner::{OwnerData, OwnerId, OwnerKind},
};

thread_local! {
    /// Former `to_owner` calls on the shipped path. T6 form B keeps this at 0.
    pub static TO_OWNER_RUNS: Cell<u32> = const { Cell::new(0) };
    /// Owners examined inside a `(name, kind)` lookup. An indexed lookup of a
    /// unique `(name, kind)` must stay at 1, not the table length.
    pub static OWNER_LOOKUP_STEPS: Cell<u32> = const { Cell::new(0) };
}

/// Compilation-unit owners of `name` whose kind is `kind`.
///
/// `locator` answers which source files declare the name. When it has no
/// match, `paid_files` are searched for macro-generated owners.
pub fn locate_cu_owners(
    db: &dyn HirDefDb,
    locator: &design_graph::UnitCatalog,
    paid_files: &[FileId],
    name: &str,
    kind: UnitKind,
) -> Vec<OwnerId> {
    locate_cu_owners_matching(db, locator, paid_files, name, |unit_kind| unit_kind == kind)
}

/// Compilation-unit owners of `name` whose kind satisfies `matches`.
pub fn locate_cu_owners_matching(
    db: &dyn HirDefDb,
    locator: &design_graph::UnitCatalog,
    paid_files: &[FileId],
    name: &str,
    matches: impl Fn(UnitKind) -> bool,
) -> Vec<OwnerId> {
    let located = located_files(locator, name, &matches);
    if !located.is_empty() {
        return located
            .into_iter()
            .flat_map(|file| cu_owners_named_in_file(db, file, name, &matches))
            .collect();
    }
    let mut files: Vec<FileId> = paid_files.to_vec();
    files.sort_by_key(|file| file.index());
    files.dedup();
    files.into_iter().flat_map(|file| cu_owners_named_in_macros(db, file, name, &matches)).collect()
}

/// Every compilation-unit package owner L0 locates. Source files only.
pub fn locate_package_owners(
    db: &dyn HirDefDb,
    locator: &design_graph::UnitCatalog,
) -> Vec<OwnerId> {
    let mut files = Vec::new();
    let mut seen = FxHashSet::default();
    for unit in locator.packages() {
        if seen.insert(unit.file) {
            files.push(unit.file);
        }
    }
    files
        .into_iter()
        .flat_map(|file| cu_owners_of_kind_in_hir(db, HirFileId::File(file), UnitKind::Package))
        .collect()
}

fn located_files(
    locator: &design_graph::UnitCatalog,
    name: &str,
    matches: &impl Fn(UnitKind) -> bool,
) -> Vec<FileId> {
    let mut files = Vec::new();
    let mut seen = FxHashSet::default();
    for unit in locator.type_units_named(name).into_vec() {
        if matches(unit.kind) && seen.insert(unit.file) {
            files.push(unit.file);
        }
    }
    files
}

fn cu_owners_named_in_file(
    db: &dyn HirDefDb,
    file: FileId,
    name: &str,
    matches: &impl Fn(UnitKind) -> bool,
) -> Vec<OwnerId> {
    cu_owners_named_in_hir(db, HirFileId::File(file), name, matches)
}

fn cu_owners_named_in_macros(
    db: &dyn HirDefDb,
    file: FileId,
    name: &str,
    matches: &impl Fn(UnitKind) -> bool,
) -> Vec<OwnerId> {
    macro_files_for_file(db, file)
        .into_iter()
        .flat_map(|macro_file| {
            cu_owners_named_in_hir(db, HirFileId::Macro(macro_file), name, matches)
        })
        .collect()
}

fn cu_owners_named_in_hir(
    db: &dyn HirDefDb,
    file: HirFileId,
    name: &str,
    matches: &impl Fn(UnitKind) -> bool,
) -> Vec<OwnerId> {
    let table = db.owner_table(file);
    let file_owner = table.file_owner();
    let mut owners = Vec::new();
    for owner_kind in [OwnerKind::Module, OwnerKind::Checker, OwnerKind::Covergroup] {
        for id in table.owners_named(name, owner_kind) {
            OWNER_LOOKUP_STEPS.with(|steps| steps.set(steps.get() + 1));
            let Some(owner) = table.owner(*id) else {
                continue;
            };
            if owner.parent != file_owner {
                continue;
            }
            let Some(kind) = unit_kind_of(owner) else {
                continue;
            };
            if matches(kind) {
                owners.push(owner.id);
            }
        }
    }
    owners
}

fn cu_owners_of_kind_in_hir(db: &dyn HirDefDb, file: HirFileId, kind: UnitKind) -> Vec<OwnerId> {
    let table = db.owner_table(file);
    let file_owner = table.file_owner();
    table
        .owners()
        .iter()
        .filter_map(|owner| {
            (owner.parent == file_owner && owner_matches_unit_kind(owner, kind)).then_some(owner.id)
        })
        .collect()
}

fn unit_kind_of(owner: &OwnerData) -> Option<UnitKind> {
    match owner.kind {
        OwnerKind::Module => match owner.module_kind {
            Some(ModuleKind::Module) => Some(UnitKind::Module),
            Some(ModuleKind::Interface) => Some(UnitKind::Interface),
            Some(ModuleKind::Package) => Some(UnitKind::Package),
            Some(ModuleKind::Program) => Some(UnitKind::Program),
            None => None,
        },
        OwnerKind::Checker => Some(UnitKind::Checker),
        OwnerKind::Covergroup => Some(UnitKind::Covergroup),
        _ => None,
    }
}

fn owner_matches_unit_kind(owner: &OwnerData, kind: UnitKind) -> bool {
    match kind {
        UnitKind::Module => {
            owner.kind == OwnerKind::Module && owner.module_kind == Some(ModuleKind::Module)
        }
        UnitKind::Interface => {
            owner.kind == OwnerKind::Module && owner.module_kind == Some(ModuleKind::Interface)
        }
        UnitKind::Package => {
            owner.kind == OwnerKind::Module && owner.module_kind == Some(ModuleKind::Package)
        }
        UnitKind::Program => {
            owner.kind == OwnerKind::Module && owner.module_kind == Some(ModuleKind::Program)
        }
        UnitKind::Checker => owner.kind == OwnerKind::Checker,
        UnitKind::Covergroup => owner.kind == OwnerKind::Covergroup,
    }
}

/// Fold a source-only graph for tests. Not a product and not a production path.
pub fn test_graph(db: &dyn HirDefDb) -> design_graph::UnitCatalog {
    design_graph::UnitCatalog::fold(db, &design_graph::GeneratedUnits::default())
}

/// Test-only resolution context over [`test_graph`].
pub fn test_resolution(db: &dyn HirDefDb) -> triomphe::Arc<crate::pathres::ResolutionContext> {
    crate::pathres::ResolutionContext::from_graph(db, triomphe::Arc::new(test_graph(db)))
}

pub fn test_module_owner(db: &dyn HirDefDb, name: &str) -> OwnerId {
    let graph = test_graph(db);
    crate::symbol::Resolution::from_candidates(locate_cu_owners(
        db,
        &graph,
        &[],
        name,
        UnitKind::Module,
    ))
    .unique()
    .unwrap_or_else(|| panic!("{name} should be a unique module owner"))
}

pub fn test_package_owner(db: &dyn HirDefDb, name: &str) -> OwnerId {
    let graph = test_graph(db);
    crate::symbol::Resolution::from_candidates(locate_cu_owners(
        db,
        &graph,
        &[],
        name,
        UnitKind::Package,
    ))
    .unique()
    .unwrap_or_else(|| panic!("{name} should be a unique package owner"))
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
    use design_graph::{GeneratedUnits, UnitCatalog, UnitId, UnitKind, UnitMeta, UnitOrigin};
    use preproc_expand::db::PreprocDb;
    use rustc_hash::FxHashSet;
    use smol_str::SmolStr;
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use super::{test_graph, test_module_owner, test_package_owner};
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

    fn db_with_text(text: &str) -> TestDb {
        let top_path = {
            let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
            AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/rtl/top.sv")))
        };
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path));
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
        db.set_file_text_with_durability(TOP, Arc::from(text), Durability::LOW);
        db
    }

    #[test]
    fn fold_joins_source_units() {
        let db = db_with_text("module top;\nendmodule\npackage p;\nendpackage\n");
        let graph = test_graph(&db);
        assert!(graph.modules_named("top").unique().is_some());
        assert!(graph.packages_named("p").unique().is_some());
        assert!(graph.modules_named("missing").is_unresolved());
    }

    #[test]
    fn fold_appends_generated_units() {
        let db = db_with_text("module top;\nendmodule\n");
        let generated_id =
            UnitId { file: TOP, name: SmolStr::new("foo"), kind: UnitKind::Module, ordinal: 0 };
        let mut generated = GeneratedUnits::default();
        let mut meta = rustc_hash::FxHashMap::default();
        meta.insert(
            generated_id.clone(),
            UnitMeta {
                kind: UnitKind::Module,
                origin: UnitOrigin::Generated,
                header_fingerprint: 0,
            },
        );
        generated.replace_file(TOP, 0, Box::new([generated_id.clone()]), meta);
        let graph = UnitCatalog::fold(&db, &generated);
        assert_eq!(graph.origin(&generated_id), Some(UnitOrigin::Generated));
        assert!(graph.modules_named("foo").unique().is_some());
        assert!(graph.modules_named("top").unique().is_some());
    }

    #[test]
    fn unique_name_lookup_does_not_scan_the_owner_table() {
        let mut text = String::new();
        for index in 0..40 {
            text.push_str(&format!("module m{index};\nendmodule\n"));
        }
        text.push_str("package p;\nendpackage\n");
        let db = db_with_text(&text);
        super::OWNER_LOOKUP_STEPS.with(|steps| steps.set(0));
        let owner = test_package_owner(&db, "p");
        let steps = super::OWNER_LOOKUP_STEPS.with(std::cell::Cell::get);
        assert_eq!(owner.name(&db).as_deref(), Some("p"));
        assert_eq!(
            steps, 1,
            "a unique (name, kind) must not walk the other owners (steps={steps})"
        );
    }

    #[test]
    fn to_owner_projects_the_ordinalth_cu_match() {
        let db = db_with_text("module top;\nendmodule\n");
        let owner = test_module_owner(&db, "top");
        assert_eq!(owner.name(&db).as_deref(), Some("top"));
    }

    #[test]
    fn to_owner_skips_nested_modules() {
        let db = db_with_text(
            "module outer;\n  module inner;\n  endmodule\nendmodule\nmodule inner;\nendmodule\n",
        );
        let graph = test_graph(&db);
        let inner = graph.modules_named("inner").unique().expect("one CU inner");
        assert_eq!(inner.ordinal, 0);
        let owner = crate::symbol::Resolution::from_candidates(super::locate_cu_owners(
            &db,
            &graph,
            &[],
            "inner",
            UnitKind::Module,
        ))
        .unique()
        .expect("CU inner projects");
        assert_eq!(owner.name(&db).as_deref(), Some("inner"));
        assert_eq!(
            owner.parent(&db).map(|parent| parent.kind(&db)),
            Some(crate::owner::OwnerKind::File)
        );
    }
}
