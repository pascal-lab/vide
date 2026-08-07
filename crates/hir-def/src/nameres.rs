//! On-demand name-scope construction keyed by canonical semantic owners.
//!
//! Legacy file/module/generate wrappers are projected to `OwnerId` at the
//! database edge. The tracked query itself has one identity space and depends
//! only on the matching ItemTree or Body owner store.
use base_db::salsa;
use triomphe::Arc;

use crate::{db::HirDefDb, owner::OwnerId, scope::build_owner_scope, symbol::NameScope};

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn scope_for(db: &dyn HirDefDb, owner: OwnerId) -> Arc<NameScope> {
    Arc::new(build_owner_scope(db, owner))
}

pub(crate) fn set_scope_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    scope_for::set_lru_capacity(db, capacity);
}

#[cfg(test)]
mod tests {
    use base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        salsa::{self, Durability},
        source_db::{FileLoader, SourceDb, SourceFileKind, SourceRootDb},
        source_root::{SourceRoot, SourceRootId},
    };
    use la_arena::{Idx, RawIdx};
    use preproc_expand::{db::PreprocDb, file::HirFileId};
    use rustc_hash::FxHashSet;
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use crate::{
        Ident,
        container::{ScopeChain, SubroutineParent, SubroutineScope},
        db::HirDefDb,
        module::{ModuleId, generate::GenerateBlockId},
        symbol::{DefKind, NameContext},
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

    impl std::fmt::Debug for TestDb {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

        let preprocess = PreprocessConfig::default();
        let project_config = ProjectConfig::new(
            vec![Some(PROFILE)],
            vec![CompilationProfile {
                source_roots: vec![ROOT],
                top_modules: Vec::new(),
                preprocess: preprocess.clone(),
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

    fn ident(name: &str) -> Ident {
        Ident::new(name)
    }

    #[test]
    fn scope_chain_is_ordered_from_inner_scope_to_file_scope() {
        let db = db_with_root_text("module top; endmodule\n");
        let file_id = HirFileId::File(TOP);
        let module_id = ModuleId::new(file_id, Idx::from_raw(RawIdx::from(0)));
        let module_owner = module_id.owner(&db).expect("module owner");
        let file_owner = db.owner_table(file_id).file_owner().expect("file owner");
        let chain = ScopeChain::from_inner(&db, module_owner);

        assert_eq!(chain.ids(), &[module_owner, file_owner]);
    }

    #[test]
    fn scope_for_builds_only_the_requested_scope() {
        let db = db_with_root_text("module top;\n  function void f(); endfunction\nendmodule\n");
        let file_id = HirFileId::File(TOP);
        let scope = db.scope_for(db.owner_table(file_id).file_owner().expect("file owner"));
        assert!(
            scope
                .lookup(NameContext::Type, &ident("top"))
                .iter()
                .any(|def_id| { def_id.kind(&db) == DefKind::Module }),
            "file scope should contain the module"
        );
        assert!(
            !scope
                .lookup(NameContext::Value, &ident("f"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Subroutine),
            "file scope alone must not contain the function body scope's name"
        );
    }

    #[test]
    fn scope_for_module_contains_subroutine_name() {
        let db = db_with_root_text("module top;\n  function void f(); endfunction\nendmodule\n");
        let file_id = HirFileId::File(TOP);
        let module_id = ModuleId::new(file_id, Idx::from_raw(RawIdx::from(0)));
        let scope = db.scope_for(module_id.owner(&db).expect("module owner"));
        assert!(
            scope
                .lookup(NameContext::Value, &ident("f"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Subroutine),
            "module scope should contain its subroutine names"
        );
    }

    #[test]
    fn scope_for_subroutine_builds_on_demand() {
        let db = db_with_root_text(
            "module top;\n  function void f();\n    logic x;\n  endfunction\nendmodule\n",
        );
        let file_id = HirFileId::File(TOP);
        let module_id = ModuleId::new(file_id, Idx::from_raw(RawIdx::from(0)));
        let subroutine_id = SubroutineScope::new(
            SubroutineParent::Module(module_id),
            Idx::from_raw(RawIdx::from(0)),
        );
        let scope = db.scope_for(subroutine_id.owner(&db).expect("subroutine owner"));
        assert!(
            scope
                .lookup(NameContext::Value, &ident("x"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Variable),
            "subroutine scope should contain its body declarations"
        );
    }

    #[test]
    fn scope_for_generate_block_contains_items() {
        let db = db_with_root_text(
            "module top;\n  generate\n    if (1) begin : g\n      logic g_sig;\n    end\n  endgenerate\nendmodule\n",
        );
        let file_id = HirFileId::File(TOP);
        let module_id = ModuleId::new(file_id, Idx::from_raw(RawIdx::from(0)));
        let module = db.module_with_source_map(module_id);
        let mut block_id = None;
        for (_, region) in module.generate_regions.iter() {
            for item in &region.items {
                if let crate::body::BodyItem::GenerateBlockId(id) = item {
                    block_id = Some(id.clone());
                }
            }
        }
        let block_id: GenerateBlockId = block_id.expect("generate block should lower");
        let scope = db.scope_for(block_id.owner(&db).expect("generate owner"));
        assert!(
            scope
                .lookup(NameContext::Value, &ident("g_sig"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Variable),
            "generate block scope should contain its declarations"
        );
    }
}
