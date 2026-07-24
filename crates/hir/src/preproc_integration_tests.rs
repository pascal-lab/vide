use std::fmt;

use rustc_hash::FxHashSet;
use triomphe::Arc;
use utils::{
    get::Get,
    line_index::TextSize,
    paths::{AbsPathBuf, Utf8PathBuf},
};
use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

use crate::{
    base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{
            CompilationProfile, CompilationProfileId, Predefine, PreprocessConfig, ProjectConfig,
        },
        salsa::{self, Durability},
        source_db::{
            FileLoader, SourceDb, SourceDbStorage, SourceFileKind, SourceRootDb,
            SourceRootDbStorage,
        },
        source_root::{SourceRoot, SourceRootId},
    },
    container::{InFile, ScopeId},
    db::{HirDefDb, HirDefDbStorage, InternDbStorage, PreprocDbStorage},
    file::HirFileId,
    hir_def::module::ModuleId,
    macro_file::macro_files_at_offset,
    preproc::diagnostic_target_for_range,
    source_map::IsSrc,
};

const TOP: FileId = FileId::from_raw(0);
const ROOT: SourceRootId = SourceRootId(0);
const PROFILE: CompilationProfileId = CompilationProfileId(0);

#[salsa::database(
    SourceDbStorage,
    SourceRootDbStorage,
    PreprocDbStorage,
    InternDbStorage,
    HirDefDbStorage
)]
#[derive(Default)]
struct TestDb {
    storage: salsa::Storage<Self>,
}

impl salsa::Database for TestDb {}

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
    db_with_root_text_and_predefines(root_text, Vec::new())
}

fn db_with_root_text_and_predefines(root_text: &str, predefines: Vec<Predefine>) -> TestDb {
    let top_path = abs_path("rtl/top.v");
    let mut file_set = FileSet::default();
    file_set.insert(TOP, VfsPath::from(top_path.clone()));
    let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);
    let preprocess = PreprocessConfig { predefines, include_dirs: Vec::new() };
    let project_config = ProjectConfig::new(
        vec![Some(PROFILE)],
        vec![CompilationProfile { source_roots: vec![ROOT], top_modules: Vec::new(), preprocess }],
    );
    let mut files = FxHashSet::default();
    files.insert(TOP);

    let mut db = TestDb::default();
    db.set_files_with_durability(Box::new(files), Durability::HIGH);
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

fn offset(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).unwrap()).unwrap())
}

#[test]
fn macro_generated_declaration_hir_range_resolves_to_diagnostic_target() {
    let root_text = r#"`define MAKE_DECL(name) logic name;
module top;
`MAKE_DECL(generated)
endmodule
"#;
    let db = db_with_root_text(root_text);
    let (hir_file, _) = db.hir_file_with_source_map(TOP.into());
    let (local_module_id, _) = hir_file.modules.iter().next().unwrap();
    let module_id: ModuleId = InFile::new(TOP.into(), local_module_id);
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (declaration_id, _) =
        module.declarations.iter().next().expect("generated declaration should lower to HIR");
    let declaration_src = module_src_map
        .get(declaration_id)
        .expect("generated declaration should keep a source-map range");

    let target =
        diagnostic_target_for_range(&db, TOP, declaration_src.range()).unwrap().target.unwrap();

    assert!(matches!(target.origin, crate::macro_file::Origin::MacroBody { .. }));
}

#[test]
fn diagnostic_target_for_unbacked_predefine_expansion_fails_closed() {
    let root_text = r#"module top;
`MAKE_CHILD
endmodule
"#;
    let db =
        db_with_root_text_and_predefines(root_text, vec![Predefine::new("MAKE_CHILD=child u();")]);
    let (hir_file, _) = db.hir_file_with_source_map(TOP.into());
    let (local_module_id, _) = hir_file.modules.iter().next().unwrap();
    let module_id: ModuleId = InFile::new(TOP.into(), local_module_id);
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (instantiation_id, _) = module
        .instantiations
        .iter()
        .next()
        .expect("predefine expansion should lower to a module instantiation");
    let instantiation_src = module_src_map
        .get(instantiation_id)
        .expect("generated instantiation should keep a source-map range");

    let target = diagnostic_target_for_range(&db, TOP, instantiation_src.range()).unwrap();

    assert!(target.covered);
    assert!(target.target.is_none(), "unbacked predefine diagnostic target should fail closed");
}

#[test]
fn macro_expanded_module_keeps_macro_hir_file_id() {
    let root_text = "`define DECL module from_macro; endmodule\n`DECL\n";
    let db = db_with_root_text(root_text);
    let macro_file = macro_files_at_offset(&db, TOP, offset(root_text, "`DECL"))
        .pop()
        .expect("macro call should expand");
    let hir_file_id = HirFileId::Macro(macro_file);

    let (hir_file, _) = db.hir_file_with_source_map(hir_file_id);
    let (local_module_id, _) =
        hir_file.modules.iter().next().expect("macro expansion should lower a module");
    let module_id = InFile::new(hir_file_id, local_module_id);

    assert_eq!(ScopeId::Module(module_id).file_id(&db), hir_file_id);
    assert_eq!(module_id.file_id.source_file_id(&db), Some(TOP));
}
