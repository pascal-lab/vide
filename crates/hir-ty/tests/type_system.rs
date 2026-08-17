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
    aggregate::ClassMemberKind,
    constraint::Constraint,
    container::OwnerRef,
    covergroup::CoverageBinInitializer,
    db::HirDefDb,
    expr::{
        Expr,
        data_ty::{DataTy, TypePathKind},
    },
    owner::OwnerId,
    pathres::{ResolutionContext, resolve_name, resolve_path},
    symbol::{NameContext, Resolution},
    unit::ToOwner,
};
use hir_ty::{Compatibility, Type, TypeSystem, db::TyDb, display::HirDisplay};
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

fn type_of_name(db: &TestDb, module: OwnerId, name: &str, context: NameContext) -> Type {
    let resolution =
        resolve_name(db, &ResolutionContext::from_db(db), module, &ident(name), context);
    assert!(!resolution.is_unresolved(), "{name} should resolve");
    TypeSystem::new(db).type_of_resolution(resolution)
}

fn type_of_path(db: &TestDb, module: OwnerId, segments: &[&str]) -> Type {
    let path = segments.iter().map(|segment| ident(segment)).collect::<Vec<_>>();
    let resolution =
        resolve_path(db, &ResolutionContext::from_db(db), module, &path, NameContext::Value);
    assert!(!resolution.is_unresolved(), "path {segments:?} should resolve");
    TypeSystem::new(db).type_of_resolution(resolution)
}

fn display_type(db: &TestDb, ty: &Type) -> String {
    TypeSystem::new(db).display_source(ty).expect("formatting a type into a String should not fail")
}

#[test]
fn semantic_types_render_through_the_public_interface() {
    let db = db_with_root_text(
        r#"
module m;
  typedef enum { A, B } state_t;
  typedef union packed { logic [7:0] byte_v; int int_v; } payload_u;
  logic queue_var[$];
  logic bounded_queue[$:4];
  logic assoc_var[string];
  logic dyn_var[];
  event ev;
  chandle handle;
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let rendered = [
        type_of_name(&db, module, "state_t", NameContext::Type),
        type_of_name(&db, module, "payload_u", NameContext::Type),
        type_of_name(&db, module, "queue_var", NameContext::Value),
        type_of_name(&db, module, "bounded_queue", NameContext::Value),
        type_of_name(&db, module, "assoc_var", NameContext::Value),
        type_of_name(&db, module, "dyn_var", NameContext::Value),
        type_of_name(&db, module, "ev", NameContext::Value),
        type_of_name(&db, module, "handle", NameContext::Value),
    ]
    .map(|ty| display_type(&db, &ty));

    assert_eq!(
        rendered,
        [
            "state_t",
            "payload_u",
            "logic [$]",
            "logic [$:4]",
            "logic [string]",
            "logic []",
            "event",
            "chandle",
        ]
    );
}

#[test]
fn members_and_compatibility_hide_classification_and_width_calculation() {
    let db = db_with_root_text(
        r#"
module m;
  typedef struct packed { logic flag; logic [2:0] code; } payload_t;
  payload_t payload;
  logic [1 + 2:0] expression_width;
  logic [3:0] four_bits;
  logic [7:0] eight_bits;
  real real_value;
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let types = TypeSystem::new(&db);
    let payload = type_of_name(&db, module, "payload", NameContext::Value);
    let member_names = types
        .members(&payload)
        .into_iter()
        .map(|member| member.into_name().to_string())
        .collect::<Vec<_>>();
    assert_eq!(member_names, ["flag", "code"]);

    let expression_width = type_of_name(&db, module, "expression_width", NameContext::Value);
    let four_bits = type_of_name(&db, module, "four_bits", NameContext::Value);
    let eight_bits = type_of_name(&db, module, "eight_bits", NameContext::Value);
    let real_value = type_of_name(&db, module, "real_value", NameContext::Value);
    assert_eq!(types.compatibility(&expression_width, &four_bits), Compatibility::Compatible);
    assert_eq!(types.compatibility(&four_bits, &eight_bits), Compatibility::Incompatible);
    assert_eq!(types.compatibility(&four_bits, &real_value), Compatibility::Incompatible);
    assert_eq!(
        types.compatibility(&four_bits, &types.type_of_resolution(Resolution::Unresolved)),
        Compatibility::Unknown
    );
}

#[test]
fn struct_member_dimensions_are_part_of_member_type() {
    let db = db_with_root_text(
        r#"
module m;
  typedef struct {
    logic data[];
    int initialized = 1;
    rand logic random_value;
  } payload_t;
  payload_t payload;
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let types = TypeSystem::new(&db);
    let payload = type_of_name(&db, module, "payload", NameContext::Value);
    let members = types.members(&payload);
    assert_eq!(
        members.iter().map(|member| member.name().as_str()).collect::<Vec<_>>(),
        ["data", "initialized", "random_value"]
    );
    assert_eq!(
        types.display_source(members[0].ty()).expect("member type should render"),
        "logic []"
    );
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
fn wildcard_dimension_is_preserved_as_associative_array() {
    let db = db_with_root_text(
        r#"
module m;
  logic values[*];
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let types = TypeSystem::new(&db);
    let values = type_of_name(&db, module, "values", NameContext::Value);
    assert_eq!(
        types.display_source(&values).expect("wildcard array type should render"),
        "logic [*]"
    );
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
    let covergroup = hir_def::unit::test_graph(&db)
        .type_units_named("cg")
        .unique()
        .expect("covergroup should be on the graph")
        .to_owner(&db)
        .expect("covergroup should project");
    let body = db.body(covergroup);
    let definition = body.covergroups.values().next().expect("covergroup should lower");
    let coverpoint = &body.coverpoints[definition.coverpoints[0]];
    assert_eq!(coverpoint.bins.len(), 1);
    assert!(matches!(coverpoint.bins[0].initializer, CoverageBinInitializer::Ranges { .. }));
    assert!(coverpoint.bins[0].size.is_some());
}

#[test]
fn class_declaration_preserves_base_and_member_kinds() {
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
    let module = module_id(&db, "m");
    let body = db.body(module);
    let class = body.classes.values().next().expect("class declaration should lower");
    assert_eq!(class.name.as_deref(), Some("C"));
    assert_eq!(class.base_class_name.as_deref(), Some("Base"));
    assert_eq!(class.members.len(), 2);
    assert_eq!(class.members[0].kind, ClassMemberKind::Property);
    assert_eq!(class.members[1].kind, ClassMemberKind::Method);
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

    let value = type_of_name(&db, module, "value", NameContext::Value);
    assert!(
        value.diagnostics().is_empty(),
        "qualified type should resolve: {:?}",
        value.diagnostics()
    );
}

#[test]
fn type_path_selectors_are_explicit_recovery() {
    let db = db_with_root_text(
        r#"
module m;
  typedef logic t;
  t[0] value;
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let value = type_of_name(&db, module, "value", NameContext::Value);
    assert_eq!(
        value.diagnostics(),
        &[hir_ty::TypeDiagnostic::InvalidTypePath(
            hir_def::expr::data_ty::TypePathRecovery::Selectors
        )]
    );
}

#[test]
fn struct_data_type_has_no_type_diagnostic() {
    let db = db_with_root_text(
        r#"
module m;
  struct { logic x; } value;
endmodule
"#,
    );
    let module = module_id(&db, "m");
    let value = type_of_name(&db, module, "value", NameContext::Value);

    assert!(value.diagnostics().is_empty());
}

#[test]
fn definition_backed_types_render_without_exposing_definition_kinds() {
    let db = db_with_root_text(
        r#"
interface bus_if;
  wire clk;
  modport host(input clk);
endinterface

program p;
endprogram

module top;
  bus_if u_if();
  p u_p();
endmodule
"#,
    );
    let top = module_id(&db, "top");
    assert_eq!(display_type(&db, &type_of_path(&db, top, &["u_if"])), "virtual interface bus_if");
    assert_eq!(
        display_type(&db, &type_of_path(&db, top, &["u_if", "host"])),
        "virtual interface bus_if.host"
    );
    assert_eq!(display_type(&db, &type_of_path(&db, top, &["u_p"])), "p");
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
