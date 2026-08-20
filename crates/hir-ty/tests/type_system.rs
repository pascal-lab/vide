use std::fmt;

use base_db::{
    diagnostics_config::DiagnosticsConfig,
    project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
    salsa::{self, Durability},
    source_db::{FileLoader, SourceDb, SourceFileKind, SourceRootDb},
    source_root::{SourceRoot, SourceRootId},
};
use hir_def::{
    Ident,
    constraint::Constraint,
    container::OwnerRef,
    covergroup::CoverageBinInitializer,
    db::HirDefDb,
    expr::{
        Expr,
        data_ty::{DataTy, TypePathKind},
    },
    owner::OwnerId,
};
use hir_ty::{db::TyDb, display::HirDisplay};
use preproc_expand::db::PreprocDb;
use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::paths::{AbsPathBuf, Utf8PathBuf};
use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

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
impl hir_def::db::DesignGraphDb for TestDb {}

#[salsa::db]
impl HirDefDb for TestDb {}

#[salsa::db]
impl TyDb for TestDb {}
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
    db.set_file_kind_with_durability(TOP, SourceFileKind::SystemVerilog, Durability::LOW);
    db.set_file_text_with_durability(TOP, Arc::from(root_text), Durability::LOW);
    db
}

fn abs_path(path: &str) -> AbsPathBuf {
    let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
    AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
}

fn ident(name: &str) -> Ident {
    SmolStr::new(name)
}

fn module_id(db: &TestDb, name: &str) -> OwnerId {
    hir_def::unit::test_module_owner(db, name)
}

#[test]
fn enum_definition_preserves_base_members_and_initializers() {
    let db = db_with_root_text(
        r#"
module m;
  typedef enum logic [1:0] {
    Idle = 2'd0,
    Busy,
    Error = 2'd3
  } state_t;
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let body = db.body(module);
    let enum_def = body.enums.values().next().expect("enum definition should be lowered");
    assert!(enum_def.base_ty.is_some(), "enum base type must be retained");
    assert_eq!(
        enum_def.members.iter().map(|member| member.name.as_deref()).collect::<Vec<_>>(),
        [Some("Idle"), Some("Busy"), Some("Error")]
    );
    assert!(enum_def.members[0].initializer.is_some());
    assert!(enum_def.members[1].initializer.is_none());
    assert!(enum_def.members[2].initializer.is_some());
}

#[test]
fn constraint_declaration_preserves_dist_and_nested_items() {
    let db = db_with_root_text(
        r#"
module m;
  logic x;
  constraint c {
    x dist { 1 := 2, default };
    unique { x };
  }
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let body = db.body(module);
    let definition =
        body.constraint_defs.values().next().expect("constraint declaration should be lowered");
    assert_eq!(definition.name.as_deref(), Some("c"));
    let Constraint::Block(items) = &body.constraints[definition.constraint] else {
        panic!("constraint declaration should lower to a block");
    };
    assert_eq!(items.len(), 2);
    let Constraint::Expression { expr, .. } = body.constraints[items[0]] else {
        panic!("distribution item should lower as an expression constraint");
    };
    let Expr::Dist { distribution, .. } = &body.exprs[expr] else {
        panic!("expression-or-dist should preserve its distribution");
    };
    assert_eq!(distribution.items.len(), 2);
    assert!(matches!(body.constraints[items[items.len() - 1]], Constraint::Uniqueness { .. }));
}

#[test]
fn coverpoint_bins_preserve_sample_expression_and_ranges() {
    let db = db_with_root_text(
        r#"
covergroup cg;
  cp: coverpoint 1 {
    bins low[2] = {[0:3]};
  }
endgroup
module m;
endmodule
"#,
    );
    let table = db.owner_table(preproc_expand::file::HirFileId::File(TOP));
    let covergroup = *table
        .owners_named("cg", hir_def::owner::OwnerKind::Covergroup)
        .first()
        .expect("covergroup should project");
    let body = db.body(covergroup);
    let definition = body.covergroups.values().next().expect("covergroup should lower");
    let coverpoint = &body.coverpoints[definition.coverpoints[0]];
    assert_eq!(coverpoint.bins.len(), 1);
    assert!(matches!(coverpoint.bins[0].initializer, CoverageBinInitializer::Ranges { .. }));
    assert!(coverpoint.bins[0].size.is_some());
}

#[test]
fn qualified_type_paths_preserve_separator_and_source_projection() {
    let db = db_with_root_text(
        r#"
package p;
  typedef logic t;
endpackage

module m;
  p::t value;
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let lowered = db.body_with_source_map(module);
    let type_ref = lowered
        .data_ref()
        .declarations
        .iter()
        .find_map(|(_, declaration)| match declaration.ty() {
            DataTy::Named(type_ref) => Some(type_ref),
            _ => None,
        })
        .expect("the value declaration should retain its named type");

    assert_eq!(type_ref.path_kind(), TypePathKind::Package);
    assert_eq!(type_ref.segments(), &[ident("p"), ident("t")]);
    assert_eq!(type_ref.segment_sources().len(), type_ref.segments().len());
    let source = db
        .source_projection(module.file(&db))
        .origin(type_ref.source())
        .expect("type path source identity must project to source data");
    assert_eq!(source.file_id(), module.file(&db));
    assert!(source.full_range().is_some());
}

#[test]
fn streaming_with_range_display_preserves_with_keyword() {
    let db = db_with_root_text(
        r#"
module m(input logic [3:0] a);
  logic [3:0] x = {<<{a with [3:0]}};
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let owner = module;
    let body = db.body_with_source_map(owner);
    let (stream_id, _) = body
        .exprs
        .iter()
        .find(|(_, expr)| matches!(expr, hir_def::expr::Expr::Stream { .. }))
        .expect("streaming concatenation should lower");

    assert_eq!(OwnerRef::new(owner, stream_id).display_source(&db).unwrap(), "{<<{a with [3:0]}}");
}
