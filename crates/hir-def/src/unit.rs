//! Project a graph `UnitId` onto a file-local `OwnerId`.
//!
//! Navigation does not call this. It is the interiors seam: ports, nets,
//! hierarchical paths, types, and package export members.

use std::cell::Cell;

use design_graph::{UnitId, UnitKind};
use preproc_expand::{file::HirFileId, macro_file::macro_files_for_file};

use crate::{
    db::HirDefDb,
    module::ModuleKind,
    owner::{OwnerData, OwnerId, OwnerKind},
};

thread_local! {
    /// `to_owner` calls on the shipped path.
    pub static TO_OWNER_RUNS: Cell<u32> = const { Cell::new(0) };
    /// Those calls whose `owner_table` query body ran (paid expanded parse).
    pub static TO_OWNER_PAID_PARSE: Cell<u32> = const { Cell::new(0) };
    /// Owners examined inside [`matching_owners`]. An indexed lookup of a
    /// unique `(name, kind)` must stay at 1, not the table length.
    pub static OWNER_LOOKUP_STEPS: Cell<u32> = const { Cell::new(0) };
    pub(crate) static IN_TO_OWNER: Cell<bool> = const { Cell::new(false) };
}

pub trait ToOwner {
    fn to_owner(self, db: &dyn HirDefDb) -> Option<OwnerId>;
}

impl ToOwner for UnitId {
    fn to_owner(self, db: &dyn HirDefDb) -> Option<OwnerId> {
        TO_OWNER_RUNS.with(|runs| runs.set(runs.get() + 1));
        IN_TO_OWNER.with(|flag| flag.set(true));
        let macro_owners: Vec<OwnerId> = macro_files_for_file(db, self.file)
            .into_iter()
            .flat_map(|macro_file| {
                matching_owners(db, HirFileId::Macro(macro_file), self.name.as_str(), self.kind)
            })
            .collect();
        let result = if !macro_owners.is_empty() {
            macro_owners.into_iter().nth(self.ordinal as usize)
        } else {
            matching_owners(db, HirFileId::File(self.file), self.name.as_str(), self.kind)
                .into_iter()
                .nth(self.ordinal as usize)
        };
        IN_TO_OWNER.with(|flag| flag.set(false));
        result
    }
}

fn matching_owners(db: &dyn HirDefDb, file: HirFileId, name: &str, kind: UnitKind) -> Vec<OwnerId> {
    let table = db.owner_table(file);
    let file_owner = table.file_owner();
    let owner_kind = match kind {
        UnitKind::Module | UnitKind::Interface | UnitKind::Package | UnitKind::Program => {
            crate::owner::OwnerKind::Module
        }
        UnitKind::Checker => crate::owner::OwnerKind::Checker,
        UnitKind::Covergroup => crate::owner::OwnerKind::Covergroup,
    };
    table
        .owners_named(name, owner_kind)
        .iter()
        .filter_map(|id| {
            OWNER_LOOKUP_STEPS.with(|steps| steps.set(steps.get() + 1));
            let owner = table.owner(*id)?;
            (owner.parent == file_owner && owner_matches_unit_kind(owner, kind)).then_some(owner.id)
        })
        .collect()
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
    test_graph(db)
        .modules_named(name)
        .unique()
        .unwrap_or_else(|| panic!("{name} should be a unique module"))
        .to_owner(db)
        .unwrap_or_else(|| panic!("{name} should project to an owner"))
}

pub fn test_package_owner(db: &dyn HirDefDb, name: &str) -> OwnerId {
    test_graph(db)
        .packages_named(name)
        .unique()
        .unwrap_or_else(|| panic!("{name} should be a unique package"))
        .to_owner(db)
        .unwrap_or_else(|| panic!("{name} should project to an owner"))
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

    use super::{ToOwner, test_graph, test_module_owner, test_package_owner};
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
        let owner = inner.to_owner(&db).expect("CU inner projects");
        assert_eq!(owner.name(&db).as_deref(), Some("inner"));
        assert_eq!(
            owner.parent(&db).map(|parent| parent.kind(&db)),
            Some(crate::owner::OwnerKind::File)
        );
    }
}
