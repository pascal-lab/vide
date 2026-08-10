//! File-level aggregation of lowering diagnostics.
//!
//! Every lowering pass stores recovery diagnostics beside its position-free
//! result; this module projects their `SourceAstId` anchors and flattens them
//! for a whole file without exposing owner-specific lowering to the IDE.
//!
//! A lowerer reports a diagnostic without a root-buffer range (`range: None`)
//! when the offending syntax has no stable position in the file's display
//! coordinates — e.g. syntax inside an included buffer. Those diagnostics
//! cannot be published to an editor as-is, so the aggregate resolves a
//! conservative display range for them, in order:
//!
//! 1. the explicit range, when the lowerer attached one;
//! 2. the projected root-buffer range of the source identity;
//! 3. a zero-width marker at the start of the owning container;
//! 4. the file start, as a last resort.

use la_arena::Arena;
use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::{SyntaxKind, SyntaxTree};
use triomphe::Arc;
use utils::{
    get::GetRef,
    text_edit::{TextRange, TextSize},
};

use crate::{
    Ident,
    ast_id_map::{SourceAstId, SyntaxFileId},
    body::BodyItem,
    container::InFile,
    db::HirDefDb,
    expr::Expr,
    has_source::HasSource,
    owner::OwnerId,
    pathres::{NameRef, RefKind, before_reference, resolve_name_at, resolve_wildcard_at},
    proc::Proc,
    source_map::{LoweringDiagnostic, LoweringDiagnosticKind},
    source_projection::SourceProjection,
    symbol::NameContext,
};

#[salsa::tracked(returns(clone))]
pub(crate) fn file_lowering_diagnostics(
    db: &dyn HirDefDb,
    file: SyntaxFileId,
) -> Arc<[LoweringDiagnostic]> {
    let file_id = file.hir_file(db);
    let tree = db.parse(file_id);
    let file_owner = db.owner_table(file_id).file_owner().expect("file owner");
    let lowered_file = db.body_with_source_map(file_owner);
    let file = lowered_file.data_ref();

    let mut diagnostics = Vec::new();
    let projection = db.source_projection(file_id);
    // File-level owners have no enclosing container; the whole file is the
    // search scope for range-less diagnostics.
    collect(lowered_file.raw_diagnostics(), None, &tree, &projection, &mut diagnostics);
    collect_import_conflicts(db, file_owner, &projection, &mut diagnostics);

    // Wildcard-activation conflicts need every name reference of the file;
    // collect them once and reuse for each scope-bearing owner.
    let mut references = Vec::new();
    collect_ident_references(db, file_owner, &mut references);
    for owner in file.subroutine_owners() {
        collect_subroutine(db, owner, &tree, &projection, &mut diagnostics);
    }
    collect_wildcard_activation_conflicts(
        db,
        file_owner,
        &references,
        &projection,
        &mut diagnostics,
    );
    for owner in file.module_owners() {
        collect_module(db, owner, &tree, &projection, &mut diagnostics);
        collect_wildcard_activation_conflicts(
            db,
            owner,
            &references,
            &projection,
            &mut diagnostics,
        );
        let mut generate_owners = Vec::new();
        collect_generate_owner_ids(db, owner, &mut generate_owners);
        for generate_owner in generate_owners {
            collect_wildcard_activation_conflicts(
                db,
                generate_owner,
                &references,
                &projection,
                &mut diagnostics,
            );
        }
    }

    collect_proc_bodies(db, &file.procs, &tree, &projection, &mut diagnostics);
    Arc::from(diagnostics)
}

fn collect_module(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let lowered = db.body_with_source_map(owner);
    let owner_range = owner.source(db).map(|source| source.value.full_range());
    collect(lowered.raw_diagnostics(), owner_range, tree, projection, diagnostics);
    collect_import_conflicts(db, owner, projection, diagnostics);

    let module = lowered.data_ref();
    let src_map = lowered.source_map();
    for owner in module.subroutine_owners() {
        collect_subroutine(db, owner, tree, projection, diagnostics);
    }

    for region_id in src_map.generate_region_srcs.iter().map(|(id, _)| id) {
        let region = module.get(region_id);
        for item in &region.items {
            if let BodyItem::GenerateBlockOwner(block_id) = item {
                collect_generate_block(db, *block_id, tree, projection, diagnostics);
            }
        }
    }
    collect_proc_bodies(db, &module.procs, tree, projection, diagnostics);
}

fn collect_generate_block(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let lowered = db.body_with_source_map(owner);
    let owner_range = owner.source(db).map(|source| source.value.full_range());
    collect(lowered.raw_diagnostics(), owner_range, tree, projection, diagnostics);
    collect_import_conflicts(db, owner, projection, diagnostics);

    let block = lowered.data_ref();
    for owner in block.subroutine_owners() {
        collect_subroutine(db, owner, tree, projection, diagnostics);
    }
    for item in &block.items {
        if let BodyItem::GenerateBlockOwner(nested) = item {
            collect_generate_block(db, *nested, tree, projection, diagnostics);
        }
    }
    collect_proc_bodies(db, &block.procs, tree, projection, diagnostics);
}

fn collect_import_conflicts(
    db: &dyn HirDefDb,
    owner: OwnerId,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    // IEEE 1800-2017 26.3: an explicit import is illegal if the imported
    // identifier is declared in the same scope or explicitly imported from
    // another package. These checks are scope-level; the wildcard-import
    // conflict rules depend on the reference point and are checked by the
    // position-aware resolver instead.
    let scope = db.scope(owner);
    let named = scope
        .imports()
        .iter()
        .filter(|import| import.name.is_some())
        .collect::<SmallVec<[&crate::symbol::Import; 4]>>();
    let mut reported = SmallVec::<[SmolStr; 4]>::new();
    for import in &named {
        let name = import.name.clone().expect("named import");
        if reported.contains(&name) {
            continue;
        }
        reported.push(name.clone());
        let declared = scope.lookup(NameContext::Listing, &name).unique().is_some();
        let other_package = named
            .iter()
            .any(|other| other.name.as_ref() == Some(&name) && other.package != import.package);
        let message = if declared {
            Some(format!(
                "explicit import of '{name}' conflicts with a declaration in the same scope"
            ))
        } else if other_package {
            Some(format!(
                "explicit import of '{name}' conflicts with another explicit import in the same scope"
            ))
        } else {
            None
        };
        let Some(message) = message else { continue };
        let Some(source) = import.source else { continue };
        let origin = projection.origin(source);
        diagnostics.push(LoweringDiagnostic {
            kind: LoweringDiagnosticKind::InvalidSyntax,
            syntax_kind: origin
                .and_then(|origin| origin.kind())
                .unwrap_or(SyntaxKind::PACKAGE_IMPORT_DECLARATION),
            source: Some(source),
            range: origin.and_then(|origin| origin.full_range()),
            message: message.into(),
        });
    }
}

/// All name-reference points of one body, recursively: every `Expr::Ident`
/// with its containing owner and source position.
fn collect_ident_references(
    db: &dyn HirDefDb,
    owner: OwnerId,
    references: &mut Vec<(OwnerId, Ident, InFile<SourceAstId>)>,
) {
    let lowered = db.body_with_source_map(owner);
    let src_map = lowered.source_map();
    for (expr_id, expr) in lowered.data_ref().exprs.iter() {
        let Expr::Ident(name) = expr else { continue };
        let Some(source) = src_map.expr_srcs.hir_to_src(expr_id) else { continue };
        references.push((owner, name.clone(), InFile::new(owner.file(db), source)));
    }
    for (_, proc) in lowered.data_ref().procs.iter() {
        collect_ident_references(db, proc.owner, references);
    }
    for subroutine_owner in lowered.data_ref().subroutine_owners() {
        collect_ident_references(db, subroutine_owner, references);
    }
    for region_id in src_map.generate_region_srcs.iter().map(|(id, _)| id) {
        for item in &lowered.data_ref().get(region_id).items {
            if let BodyItem::GenerateBlockOwner(block_id) = item {
                collect_ident_references(db, *block_id, references);
            }
        }
    }
    for item in &lowered.data_ref().items {
        match item {
            BodyItem::ModuleOwner(module_id) | BodyItem::GenerateBlockOwner(module_id) => {
                collect_ident_references(db, *module_id, references);
            }
            _ => {}
        }
    }
}

/// Every generate-block owner nested below `owner`, recursively.
fn collect_generate_owner_ids(db: &dyn HirDefDb, owner: OwnerId, out: &mut Vec<OwnerId>) {
    let lowered = db.body_with_source_map(owner);
    let src_map = lowered.source_map();
    for region_id in src_map.generate_region_srcs.iter().map(|(id, _)| id) {
        for item in &lowered.data_ref().get(region_id).items {
            if let BodyItem::GenerateBlockOwner(block_id) = item {
                out.push(*block_id);
                collect_generate_owner_ids(db, *block_id, out);
            }
        }
    }
    for item in &lowered.data_ref().items {
        if let BodyItem::GenerateBlockOwner(block_id) = item {
            out.push(*block_id);
            collect_generate_owner_ids(db, *block_id, out);
        }
    }
}

/// IEEE 1800-2017 26.3: once a reference makes a wildcard-imported symbol
/// locally visible in a scope, any later declaration of the same name in that
/// scope is illegal.
fn collect_wildcard_activation_conflicts(
    db: &dyn HirDefDb,
    owner: OwnerId,
    references: &[(OwnerId, Ident, InFile<SourceAstId>)],
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let scope = db.scope(owner);
    if !scope.imports().iter().any(|import| import.name.is_none()) {
        return;
    }
    for (name, defs) in scope.iter_listing() {
        for def_id in defs {
            let Some(declaration_source) = def_id.primary_origin(db).loc(db).clone().source_ast(db)
            else {
                continue;
            };
            let activated = references.iter().any(|(ref_owner, ref_name, ref_position)| {
                if ref_name != name {
                    return false;
                }
                let reference = NameRef { position: *ref_position, kind: RefKind::Value };
                let resolved = [NameContext::Type, NameContext::Value].into_iter().any(|ctx| {
                    let resolved = resolve_name_at(db, *ref_owner, name, ctx, Some(&reference));
                    if resolved.is_unresolved() {
                        return false;
                    }
                    let (wildcard, activated_scope) =
                        resolve_wildcard_at(db, *ref_owner, name, ctx, Some(&reference));
                    activated_scope == Some(owner) && resolved == wildcard
                });
                resolved
                    && before_reference(
                        db,
                        *ref_position,
                        &NameRef { position: declaration_source, kind: RefKind::Value },
                    )
            });
            if !activated {
                continue;
            }
            let origin = projection.origin(declaration_source.value);
            diagnostics.push(LoweringDiagnostic {
                kind: LoweringDiagnosticKind::InvalidSyntax,
                syntax_kind: origin
                    .and_then(|origin| origin.kind())
                    .unwrap_or(SyntaxKind::DATA_DECLARATION),
                source: Some(declaration_source.value),
                range: origin.and_then(|origin| origin.full_range()),
                message: format!(
                    "declaration of '{name}' conflicts with the name imported via wildcard import"
                )
                .into(),
            });
        }
    }
}

fn collect_subroutine(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let lowered = db.body_with_source_map(owner);
    let owner_range = owner.source(db).map(|source| source.value.full_range());
    collect(lowered.raw_diagnostics(), owner_range, tree, projection, diagnostics);
}

fn collect_proc_bodies(
    db: &dyn HirDefDb,
    procs: &Arena<Proc>,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    for (_, proc) in procs.iter() {
        let body = db.body_with_source_map(proc.owner);
        let owner_range = proc.owner.source(db).map(|source| source.value.full_range());
        collect(body.raw_diagnostics(), owner_range, tree, projection, diagnostics);
    }
}

fn collect(
    diagnostics: &[LoweringDiagnostic],
    owner_range: Option<TextRange>,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    out: &mut Vec<LoweringDiagnostic>,
) {
    for diagnostic in diagnostics {
        let mut diagnostic = diagnostic.clone();
        if diagnostic.range.is_none() {
            diagnostic.range = diagnostic
                .source
                .and_then(|source| projection.origin(source))
                .and_then(|origin| origin.full_range())
                .or_else(|| Some(resolve_range(tree, owner_range, &diagnostic)));
        }
        out.push(diagnostic);
    }
}

/// Resolves a conservative display range without guessing from unrelated
/// syntax of the same kind.
fn resolve_range(
    _tree: &SyntaxTree,
    owner_range: Option<TextRange>,
    diagnostic: &LoweringDiagnostic,
) -> TextRange {
    diagnostic
        .range
        .or_else(|| owner_range.map(|range| TextRange::empty(range.start())))
        .unwrap_or_else(|| TextRange::empty(TextSize::new(0)))
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
    use syntax::{SyntaxKind, SyntaxTree};
    use triomphe::Arc;
    use utils::{
        paths::{AbsPathBuf, Utf8PathBuf},
        text_edit::{TextRange, TextSize},
    };
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use crate::{
        bind::{BindInstantiationKind, BindPathKind},
        db::HirDefDb,
        declaration::{Declaration, ParamDeclKind},
        dpi::{DpiExportKind, DpiImportProperty, DpiSpec},
        expr::data_ty::DataTy,
        external::{ExternModulePortList, ExternParameter},
        source_map::{LoweringDiagnostic, LoweringDiagnosticKind},
        time_units::{TimeScaleMagnitude, TimeUnitsKind},
        typedef::ForwardTypedefKind,
    };

    const TOP: FileId = FileId::from_raw(0);
    const HEADER: FileId = FileId::from_raw(1);
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

    fn db_with_files(root_text: &str, header_text: Option<&str>) -> TestDb {
        let root_path = abs_path("rtl");
        let top_path = root_path.join("top.sv");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        let mut source_files = vec![TOP];
        if header_text.is_some() {
            let header_path = root_path.join("defs.vh");
            file_set.insert(HEADER, VfsPath::from(header_path));
            source_files.push(HEADER);
        }
        let root = SourceRoot::new_local_with_source_files(file_set, source_files);
        let mut files = FxHashSet::default();
        files.insert(TOP);
        if header_text.is_some() {
            files.insert(HEADER);
        }

        let preprocess = PreprocessConfig {
            include_dirs: vec![root_path.clone()],
            ..PreprocessConfig::default()
        };
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
        if let Some(header_text) = header_text {
            let header_path = root_path.join("defs.vh");
            db.set_source_root_id_with_durability(HEADER, ROOT, Durability::LOW);
            db.set_file_path_with_durability(HEADER, Some(header_path), Durability::LOW);
            db.set_file_kind_with_durability(
                HEADER,
                SourceFileKind::IncludeHeader,
                Durability::LOW,
            );
            db.set_file_text_with_durability(HEADER, Arc::from(header_text), Durability::LOW);
        }
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    fn range_of(text: &str, needle: &str) -> TextRange {
        range_of_nth(text, needle, 0)
    }

    fn range_of_nth(text: &str, needle: &str, nth: usize) -> TextRange {
        let start = text.match_indices(needle).nth(nth).unwrap().0;
        let start = TextSize::from(u32::try_from(start).unwrap());
        TextRange::new(start, start + TextSize::of(needle))
    }

    #[test]
    fn supported_constructs_produce_no_lowering_diagnostics() {
        let text = r#"
module m;
  int x = '{default: 0};
  initial begin : blk
    int y = '{default: 0};
  end
  task automatic t;
    int z = '{default: 0};
  endtask
  struct { logic a; } value;
endmodule
"#;
        let db = db_with_files(text, None);

        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics.is_empty(),
            "supported assignment patterns and struct types must not be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn property_case_expression_is_lowered_without_diagnostic() {
        let text = r#"
module m(input logic x, y);
  property p;
    case (x)
      1'b1: y;
      default: 1'b0;
    endcase
  endproperty
endmodule
"#;
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            !diagnostics.iter().any(|diag| diag.message == "unsupported expression"),
            "property case expressions must be lowered: {diagnostics:?}"
        );
    }

    #[test]
    fn default_nettype_none_diagnoses_bare_port_header() {
        // A first port with only a direction has no previous header to
        // inherit, so the default implicit-net header must also honor
        // `default_nettype none`.
        let text = "`default_nettype none\nmodule m(output a);\nendmodule\n";
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("default_nettype none")),
            "bare output port under `default_nettype none` must be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn file_declarations_preserve_config_and_udp_structure() {
        let text = r#"
primitive p (o, i);
  table
    0 : 0 : 0;
  endtable
endprimitive
module m;
endmodule
config c;
  design m;
  default liblist work;
endconfig
"#;
        let db = db_with_files(text, None);
        let body = db.body(db.owner_table(HirFileId::File(TOP)).file_owner().unwrap());
        let udp = body.udp_decls.values().next().expect("UDP should be lowered");
        assert_eq!(udp.name.as_deref(), Some("p"));
        assert_eq!(udp.ports.len(), 2);
        assert_eq!(udp.entries.len(), 1);
        let config = body.config_decls.values().next().expect("config should be lowered");
        assert_eq!(config.name.as_deref(), Some("c"));
        assert_eq!(config.rules.len(), 1);
    }

    #[test]
    fn supported_compilation_unit_members_produce_no_lowering_diagnostics() {
        let text = r#"
int unit_data;
wire unit_net;
typedef int unit_type;
typedef class unit_forward;
typedef enum unit_forward_enum;
typedef union unit_forward_union;
typedef interface class unit_forward_interface;
typedef union tagged { int foo; } unit_tagged_union;
nettype logic unit_nettype with unit_resolution;
unit_nettype #1 unit_signal;
parameter int unit_parameter = 1;
timeunit 1ns / 1ps;
import "DPI-C" context c_import = function int imported(input int a, output logic b);
import "DPI" pure function void pure_import(input int value);
export "DPI-C" c_export = function unit_function;
export "DPI-C" task unit_task;
extern module unit_external #(parameter int width = 8, parameter type data_t = logic)
  (input logic in_a, output logic out_b);
extern primitive unit_external_primitive(output primitive_out, input primitive_in_a, primitive_in_b);
virtual class unit_class #(parameter int width = 8, parameter type data_t = logic)
  implements unit_interface;
  protected rand int value;
  local static int hidden;
  const int immutable = 1;
  randc int cycle;
  function void method();
    super.new(default);
    this.randomize with (value) { value > 0; };
    void'(unit_function());
  endfunction
  pure virtual function void prototype(input int value);
  constraint bounds { value inside {[0:$]}; }
  constraint external;
endclass
interface class unit_interface_class;
endclass
class : final final_class;
endclass
function void unit_function();
endfunction
primitive unit_primitive (o, i);
  table
    0 : 0 : 0;
  endtable
endprimitive
checker unit_checker;
endchecker
covergroup unit_covergroup;
endgroup
bind unit_module unit_bound unit_bind();
bind unit_module unit_checker::unit_checker unit_checker_bind();
module unit_module;
  unit_class created = new(8);
  unit_class copied = new created;
  unit_class created_array[] = new[3];
  unit_class initialized_array[] = new[3](created);
  unit_class empty_array[] = {};
  unit_class null_handle = null;
  unit_tagged_union tagged_value = tagged foo 1;
  int array_values [0:1];
  int array_sum = array_values.sum with (item);
  typedef struct module_forward;
  nettype logic module_nettype;
  module_nettype #2 module_signal;
  alias module_signal = module_signal;
  var logic module_variable;
  let unit_let(input logic value) = value;
  default disable iff (1'b0);
  $error(, "unit error");
  $fatal(1, "unit fatal");
  $static_assert(1'b1);
  timeunit 10ns;
  timeprecision 1ps;
  import "DPI-C" function void module_import();
  export "DPI-C" function unit_function;
  bind unit_module: unit_instance, unit_module.sub[1] unit_bound module_bind();
endmodule
interface unit_interface;
  logic unit_clock;
  clocking unit_clocking @(posedge unit_clock);
    input unit_clock;
  endclocking
  modport unit_modport(input unit_clock, clocking unit_clocking);
  extern function void external_method(input int value);
  extern forkjoin task external_task(input int value);
endinterface
config unit_config;
  design unit_module;
  default liblist work;
endconfig
program;
  task run(input int value);
  endtask
endprogram
"#;
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics.is_empty(),
            "supported compilation-unit members must not be diagnosed: {diagnostics:?}"
        );
        let file_owner = db.owner_table(HirFileId::File(TOP)).file_owner().unwrap();
        let body = db.body(file_owner);
        let parameter = body.declarations.values().find_map(|declaration| match declaration {
            Declaration::ParamDecl(parameter) if !parameter.is_port => Some(parameter),
            _ => None,
        });
        assert_eq!(parameter.map(|parameter| parameter.kind), Some(ParamDeclKind::Parameter));
        let file_forward = body
            .typedefs
            .values()
            .find(|typedef| typedef.name.as_deref() == Some("unit_forward"))
            .expect("file forward typedef should be lowered");
        assert_eq!(file_forward.ty, None);
        assert_eq!(file_forward.forward_kind, Some(ForwardTypedefKind::Class));
        for (name, kind) in [
            ("unit_forward_enum", ForwardTypedefKind::Enum),
            ("unit_forward_union", ForwardTypedefKind::Union),
            ("unit_forward_interface", ForwardTypedefKind::InterfaceClass),
        ] {
            let forward = body
                .typedefs
                .values()
                .find(|typedef| typedef.name.as_deref() == Some(name))
                .expect("file forward typedef should be lowered");
            assert_eq!(forward.forward_kind, Some(kind));
        }
        assert_eq!(body.net_type_decls.len(), 1);
        let file_net_type =
            body.net_type_decls.values().next().expect("file nettype should be lowered");
        assert_eq!(file_net_type.name, "unit_nettype");
        assert_eq!(file_net_type.with_function.as_deref(), Some("unit_resolution"));
        let file_user_net = body
            .declarations
            .values()
            .find_map(|declaration| match declaration {
                Declaration::NetDecl(net) if matches!(&net.ty, DataTy::Named(ty) if ty.segments() == ["unit_nettype"]) => {
                    Some(net)
                }
                _ => None,
            })
            .expect("file user-defined net should be lowered");
        assert!(file_user_net.net_kind.is_none());
        assert!(file_user_net.delay.is_some());
        let time_units = body.time_units.values().next().expect("file time unit should be lowered");
        assert_eq!(time_units.kind, TimeUnitsKind::Unit);
        assert_eq!(time_units.value.unit, syntax::TimeUnit::Nanoseconds);
        assert_eq!(time_units.value.magnitude, TimeScaleMagnitude::One);
        assert_eq!(time_units.precision.unwrap().unit, syntax::TimeUnit::Picoseconds);
        assert_eq!(body.bind_directives.len(), 2);
        assert_eq!(body.dpi_imports.len(), 2);
        assert_eq!(body.dpi_exports.len(), 2);
        let mut dpi_imports = body.dpi_imports.values();
        let context_import = dpi_imports.next().expect("context DPI import should be lowered");
        let pure_import = dpi_imports.next().expect("pure DPI import should be lowered");
        assert_eq!(context_import.spec, DpiSpec::DpiC);
        assert_eq!(context_import.property, Some(DpiImportProperty::Context));
        assert_eq!(context_import.c_identifier.as_deref(), Some("c_import"));
        assert_eq!(context_import.method.name.as_deref(), Some("imported"));
        assert_eq!(context_import.method.ports.len(), 2);
        assert_eq!(pure_import.spec, DpiSpec::Dpi);
        assert_eq!(pure_import.property, Some(DpiImportProperty::Pure));
        assert_eq!(body.extern_module_decls.len(), 1);
        let external =
            body.extern_module_decls.values().next().expect("extern module should be lowered");
        assert_eq!(external.kind, crate::module::ModuleKind::Module);
        assert_eq!(external.name, "unit_external");
        assert_eq!(external.parameters.len(), 2);
        assert!(matches!(external.parameters[0], ExternParameter::Value { .. }));
        assert!(matches!(external.parameters[1], ExternParameter::Type { default: Some(_), .. }));
        let Some(ExternModulePortList::Ansi(ports)) = external.ports.as_ref() else {
            panic!("extern module ANSI ports should be lowered");
        };
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].name.as_deref(), Some("in_a"));
        assert_eq!(ports[1].name.as_deref(), Some("out_b"));
        assert_eq!(body.extern_udp_decls.len(), 1);
        let external_udp =
            body.extern_udp_decls.values().next().expect("extern UDP should be lowered");
        assert_eq!(external_udp.name, "unit_external_primitive");
        assert_eq!(
            external_udp.ports.as_slice(),
            ["primitive_out", "primitive_in_a", "primitive_in_b"]
        );
        let mut dpi_exports = body.dpi_exports.values();
        let function_export = dpi_exports.next().expect("function DPI export should be lowered");
        let task_export = dpi_exports.next().expect("task DPI export should be lowered");
        assert_eq!(function_export.kind, DpiExportKind::Function);
        assert_eq!(function_export.c_identifier.as_deref(), Some("c_export"));
        assert_eq!(function_export.name, "unit_function");
        assert_eq!(task_export.kind, DpiExportKind::Task);
        assert_eq!(task_export.name, "unit_task");
        let mut binds = body.bind_directives.values();
        let hierarchy_bind = binds.next().expect("hierarchy bind should be lowered");
        let checker_bind = binds.next().expect("checker bind should be lowered");
        assert_eq!(hierarchy_bind.instantiation_kind, BindInstantiationKind::Hierarchy);
        assert_eq!(checker_bind.instantiation_kind, BindInstantiationKind::Checker);
        assert_eq!(hierarchy_bind.target.segments[0].name, "unit_module");
        let module = body.module_owners().next().expect("module owner should be lowered");
        let module_body = db.body(module);
        let module_source_map = db.body_with_source_map(module);
        let module_forward = module_body
            .typedefs
            .values()
            .find(|typedef| typedef.name.as_deref() == Some("module_forward"))
            .expect("module forward typedef should be lowered");
        assert_eq!(module_forward.ty, None);
        assert_eq!(module_forward.forward_kind, Some(ForwardTypedefKind::Struct));
        assert_eq!(module_body.net_type_decls.len(), 1);
        assert_eq!(module_body.net_type_decls.values().next().unwrap().name, "module_nettype");
        let module_user_net = module_body
            .declarations
            .values()
            .find_map(|declaration| match declaration {
                Declaration::NetDecl(net) if matches!(&net.ty, DataTy::Named(ty) if ty.segments() == ["module_nettype"]) => {
                    Some(net)
                }
                _ => None,
            })
            .expect("module user-defined net should be lowered");
        assert!(module_user_net.net_kind.is_none());
        assert!(module_user_net.delay.is_some());
        assert_eq!(module_body.net_aliases.len(), 1);
        assert_eq!(module_body.net_aliases.values().next().unwrap().nets.len(), 2);
        let module_variable = module_body
            .declarations
            .values()
            .find_map(|declaration| match declaration {
                Declaration::DataDecl(data) if data.var_kw => Some(data),
                _ => None,
            })
            .expect("module local variable should be lowered");
        assert_eq!(module_variable.decls.len(), 1);
        let let_decl =
            module_body.let_decls.values().next().expect("let declaration should be lowered");
        assert_eq!(let_decl.name, "unit_let");
        assert_eq!(let_decl.ports.len(), 1);
        assert_eq!(let_decl.ports[0].name.as_deref(), Some("value"));
        let default_disable =
            module_body.default_disable.expect("default disable declaration should be lowered");
        assert!(module_source_map.source_map().default_disable_src.is_some());
        assert!(matches!(module_body.expr(default_disable), crate::expr::Expr::Literal(_)));
        let mut elab_tasks = module_body.elab_system_tasks.values();
        let error = elab_tasks.next().expect("$error should be lowered");
        assert_eq!(error.kind, crate::elab_system_task::ElabSystemTaskKind::Error);
        assert!(matches!(
            error.arguments.as_slice(),
            [crate::elab_system_task::ElabSystemTaskArgument::Empty, ..]
        ));
        let fatal = elab_tasks.next().expect("$fatal should be lowered");
        assert_eq!(fatal.kind, crate::elab_system_task::ElabSystemTaskKind::Fatal);
        assert_eq!(fatal.arguments.len(), 2);
        let static_assert = elab_tasks.next().expect("$static_assert should be lowered");
        assert_eq!(static_assert.kind, crate::elab_system_task::ElabSystemTaskKind::StaticAssert);
        assert_eq!(static_assert.arguments.len(), 1);
        assert_eq!(module_body.time_units.len(), 2);
        assert_eq!(module_body.dpi_imports.len(), 1);
        assert_eq!(module_body.dpi_exports.len(), 1);
        let interface = body
            .module_owners()
            .find(|owner| db.body(*owner).name.as_deref() == Some("unit_interface"))
            .expect("interface owner should be lowered");
        let interface_body = db.body(interface);
        let modport = interface_body
            .modports
            .values()
            .find(|modport| modport.name.as_deref() == Some("unit_modport"))
            .expect("interface modport should be lowered");
        assert_eq!(modport.ports.len(), 2);
        assert!(modport.ports.iter().any(|port| {
            matches!(port.kind, crate::module::modport::ModportPortKind::Clocking)
        }));
        assert_eq!(interface_body.extern_interface_methods.len(), 2);
        let mut methods = interface_body.extern_interface_methods.values();
        let function = methods.next().expect("extern function should be lowered");
        let task = methods.next().expect("extern task should be lowered");
        assert_eq!(function.method.name.as_deref(), Some("external_method"));
        assert!(!function.fork_join);
        assert_eq!(function.method.ports.len(), 1);
        assert_eq!(task.method.name.as_deref(), Some("external_task"));
        assert!(task.fork_join);
        let mut module_time_units = module_body.time_units.values();
        let module_time_unit =
            module_time_units.next().expect("module time unit should be lowered");
        let module_time_precision =
            module_time_units.next().expect("module time precision should be lowered");
        assert_eq!(module_time_unit.kind, TimeUnitsKind::Unit);
        assert_eq!(module_time_unit.value.magnitude, TimeScaleMagnitude::Ten);
        assert_eq!(module_time_precision.kind, TimeUnitsKind::Precision);
        let module_bind =
            module_body.bind_directives.values().next().expect("module bind should be lowered");
        assert_eq!(module_bind.target_instances.len(), 2);
        assert_eq!(module_bind.target_instances[1].kind, BindPathKind::Hierarchical);
        assert_eq!(module_bind.target_instances[1].segments[1].selectors.len(), 1);
        assert!(module_body.exprs.values().any(|expr| {
            matches!(expr, crate::expr::Expr::NewClass { args: Some(args), .. } if args.len() == 1)
        }));
        assert!(
            module_body
                .exprs
                .values()
                .any(|expr| matches!(expr, crate::expr::Expr::CopyClass { .. }))
        );
        assert!(
            module_body.exprs.values().any(|expr| {
                matches!(expr, crate::expr::Expr::NewArray { initializer: None, .. })
            })
        );
        assert!(module_body.exprs.values().any(|expr| {
            matches!(expr, crate::expr::Expr::NewArray { initializer: Some(_), .. })
        }));
        assert!(
            module_body.exprs.values().any(|expr| matches!(expr, crate::expr::Expr::EmptyQueue))
        );
        assert!(module_body.exprs.values().any(|expr| {
            matches!(expr, crate::expr::Expr::Literal(crate::literal::Literal::Null))
        }));
        assert!(module_body.exprs.values().any(|expr| {
            matches!(expr, crate::expr::Expr::TaggedUnion { member: Some(member), expr: Some(_) } if member == "foo")
        }));
        assert!(module_body.exprs.values().any(|expr| {
            matches!(
                expr,
                crate::expr::Expr::ArrayOrRandomizeMethod {
                    with_args: Some(args),
                    constraints: None,
                    ..
                } if args.len() == 1
            )
        }));
        assert!(body.exprs.values().any(|expr| {
            matches!(expr, crate::expr::Expr::Literal(crate::literal::Literal::Unbounded))
        }));
        let class = body
            .classes
            .values()
            .find(|class| class.name.as_deref() == Some("unit_class"))
            .expect("file-level class should be lowered");
        assert_eq!(class.kind, crate::aggregate::ClassKind::Virtual);
        assert!(!class.is_final);
        assert_eq!(class.parameters.len(), 2);
        assert!(matches!(
            &class.parameters[0],
            crate::aggregate::ClassParameter::Value {
                name: Some(name),
                default: Some(_),
                is_local: false,
                ..
            } if name == "width"
        ));
        assert!(matches!(
            &class.parameters[1],
            crate::aggregate::ClassParameter::Type {
                name: Some(name),
                default: Some(_),
                restriction: None,
                is_local: false,
            } if name == "data_t"
        ));
        assert_eq!(class.implemented_interfaces.as_slice(), ["unit_interface"]);
        assert_eq!(class.members.len(), 8);
        assert!(class.members[0].ty.is_some());
        assert_eq!(
            class.members[0].property_qualifiers,
            Some(crate::aggregate::ClassPropertyQualifiers {
                is_const: false,
                is_static: false,
                random: Some(crate::aggregate::ClassRandomQualifier::Rand),
                visibility: crate::aggregate::ClassVisibility::Protected,
            })
        );
        assert_eq!(
            class.members[1].property_qualifiers.map(|qualifiers| (
                qualifiers.is_const,
                qualifiers.is_static,
                qualifiers.random,
                qualifiers.visibility,
            )),
            Some((false, true, None, crate::aggregate::ClassVisibility::Local))
        );
        assert!(class.members[2].property_qualifiers.is_some_and(|qualifiers| qualifiers.is_const));
        assert_eq!(
            class.members[3].property_qualifiers.and_then(|qualifiers| qualifiers.random),
            Some(crate::aggregate::ClassRandomQualifier::RandC)
        );
        assert!(class.members[4].method.as_ref().is_some_and(|method| method.has_body));
        let method_owner = class.members[4].owner.expect("class method owner should be lowered");
        let method_body = db.body(method_owner);
        assert!(
            method_body
                .exprs
                .values()
                .any(|expr| matches!(expr, crate::expr::Expr::SuperNewDefaulted { .. }))
        );
        assert!(method_body.exprs.values().any(|expr| {
            matches!(
                expr,
                crate::expr::Expr::ArrayOrRandomizeMethod {
                    with_args: Some(args),
                    constraints: Some(_),
                    ..
                } if args.len() == 1
            )
        }));
        assert!(
            method_body
                .stmts
                .values()
                .any(|stmt| { matches!(stmt.kind, crate::stmt::StmtKind::VoidCastedCall(_)) })
        );
        assert!(class.members[5].method.as_ref().is_some_and(|method| !method.has_body));
        assert_eq!(
            class.members[5].method_qualifiers,
            Some(crate::aggregate::ClassMethodQualifiers {
                is_pure: true,
                is_virtual: true,
                is_extern: false,
                is_static: false,
                visibility: crate::aggregate::ClassVisibility::Public,
            })
        );
        assert!(class.members[6].constraint.is_some());
        assert!(class.members[7].constraint.is_some());
        assert_eq!(body.constraint_defs.len(), 2);
        assert!(body.constraint_defs.values().any(|constraint| !constraint.prototype));
        assert!(body.constraint_defs.values().any(|constraint| constraint.prototype));
        let interface_class = body
            .classes
            .values()
            .find(|class| class.name.as_deref() == Some("unit_interface_class"))
            .expect("interface class should be lowered");
        assert_eq!(interface_class.kind, crate::aggregate::ClassKind::Interface);
        let final_class = body
            .classes
            .values()
            .find(|class| class.name.as_deref() == Some("final_class"))
            .expect("final class should be lowered");
        assert_eq!(final_class.kind, crate::aggregate::ClassKind::Class);
        assert!(final_class.is_final);
        let anonymous_program = body
            .items
            .iter()
            .find_map(|item| match item {
                crate::body::BodyItem::AnonymousProgramOwner(owner) => Some(*owner),
                _ => None,
            })
            .expect("anonymous program owner should be lowered");
        let anonymous_body = db.body(anonymous_program);
        assert!(anonymous_body.name.is_none());
        let subroutine_owner = anonymous_body
            .subroutine_owners()
            .next()
            .expect("anonymous program task should be lowered");
        assert_eq!(db.subroutine(subroutine_owner).name.as_deref(), Some("run"));
    }

    #[test]
    fn invalid_time_units_value_produces_lowering_diagnostic() {
        let db = db_with_files("timeunit 2ns;\n", None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("invalid time scale value")),
            "invalid time scale values must be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn duplicate_default_disable_produces_lowering_diagnostic() {
        let db = db_with_files(
            "module m; default disable iff (1'b0); default disable iff (1'b1); endmodule\n",
            None,
        );
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("more than one default disable")),
            "duplicate default disable declarations must be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn default_nettype_none_diagnoses_implicit_nets() {
        let text = "`default_nettype none\nmodule m(input a);\nendmodule\n";
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("default_nettype none")),
            "implicit net under `default_nettype none` must be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn wildcard_activated_then_declared_is_illegal() {
        // IEEE 1800-2017 26.3 Example 1: the reference activates p::x in the
        // module scope (the later declaration is point-filtered), so the
        // module's later declaration of x is illegal.
        let text = "package p;\nint x;\nendpackage\nmodule m;\nimport p::*;\ninitial begin : blk\n  x = 1;\nend\nint x;\nendmodule\n";
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.message.contains("conflicts with the name imported")),
            "later declaration after wildcard activation must be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn wildcard_not_activated_by_later_declaration_is_legal() {
        // The reference after the declaration binds to the declaration, so
        // the wildcard import is never activated and everything is legal.
        let text = "package p;\nint x;\nendpackage\nmodule m;\nimport p::*;\ninitial begin : blk\n  int x;\n  x = 1;\nend\nendmodule\n";
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            !diagnostics
                .iter()
                .any(|diag| diag.message.contains("conflicts with the name imported")),
            "declaration before the reference never conflicts: {diagnostics:?}"
        );
    }

    #[test]
    fn wildcard_without_reference_is_legal() {
        let text = "package p;\nint x;\nendpackage\nmodule m;\nimport p::*;\nint x;\nendmodule\n";
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            !diagnostics
                .iter()
                .any(|diag| diag.message.contains("conflicts with the name imported")),
            "a wildcard import alone must not conflict: {diagnostics:?}"
        );
    }

    #[test]
    fn explicit_import_conflicting_with_declaration_is_diagnosed() {
        let text = "package p;\nint x;\nendpackage\nmodule m;\nint x;\nimport p::x;\nendmodule\n";
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("conflicts with a declaration")),
            "explicit import of a declared name must be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn explicit_import_conflicting_across_packages_is_diagnosed() {
        let text = "package p;\nint x;\nendpackage\npackage q;\nint x;\nendpackage\nmodule m;\nimport p::x;\nimport q::x;\nendmodule\n";
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("another explicit import")),
            "explicit imports of one name from two packages must be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn legal_imports_produce_no_conflict_diagnostics() {
        let text = "package p;\nint x;\nendpackage\npackage q;\nint y;\nendpackage\nmodule m;\nimport p::x;\nimport q::*;\nendmodule\n";
        let db = db_with_files(text, None);
        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            !diagnostics.iter().any(|diag| diag.message.contains("conflicts")),
            "legal imports must not conflict: {diagnostics:?}"
        );
    }

    #[test]
    fn foreach_statement_is_lowered_without_diagnostic() {
        let text = "module m;\ninitial begin\n  foreach (arr[i]) x = 1;\nend\nendmodule\n";
        let db = db_with_files(text, None);

        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            !diagnostics.iter().any(|diag| diag.message == "unsupported statement"),
            "lowered foreach statements must not be diagnosed: {diagnostics:?}"
        );
    }

    #[test]
    fn include_buffer_struct_type_is_lowered_without_diagnostic() {
        let text = "module m;\n`include \"defs.vh\"\nendmodule\n";
        let db = db_with_files(text, Some("struct { logic a; } value;\n"));

        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        assert!(
            diagnostics.is_empty(),
            "included struct types must be lowered without diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn range_resolution_prefers_explicit_range() {
        let tree = SyntaxTree::from_text(
            "module m;\n  int x = '{default: 0};\nendmodule\n",
            "top.sv",
            "/repo/rtl/top.sv",
        );
        let explicit = TextRange::new(TextSize::new(10), TextSize::new(20));
        let diagnostic = LoweringDiagnostic {
            kind: LoweringDiagnosticKind::UnsupportedSyntax,
            syntax_kind: SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION,
            source: None,
            range: Some(explicit),
            message: "unsupported expression".into(),
        };

        assert_eq!(super::resolve_range(&tree, None, &diagnostic), explicit);
    }

    #[test]
    fn range_resolution_uses_file_start_without_owner() {
        let text = "module m;\n  int x = '{default: 0};\nendmodule\n";
        let tree = SyntaxTree::from_text(text, "top.sv", "/repo/rtl/top.sv");
        let diagnostic = LoweringDiagnostic {
            kind: LoweringDiagnosticKind::UnsupportedSyntax,
            syntax_kind: SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION,
            source: None,
            range: None,
            message: "unsupported expression".into(),
        };

        let range = super::resolve_range(&tree, None, &diagnostic);
        assert_eq!(range, TextRange::empty(TextSize::new(0)));
    }

    #[test]
    fn range_resolution_honors_owner_scope() {
        let text = "module a;\n  int x = '{default: 0};\nendmodule\nmodule b;\nendmodule\n";
        let tree = SyntaxTree::from_text(text, "top.sv", "/repo/rtl/top.sv");
        let diagnostic = LoweringDiagnostic {
            kind: LoweringDiagnosticKind::UnsupportedSyntax,
            syntax_kind: SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION,
            source: None,
            range: None,
            message: "unsupported expression".into(),
        };

        // Restricting the search to module b (which has no such node) must
        // fall through to the owner start instead of grabbing module a's node.
        let owner_range = range_of(text, "module b;");
        let range = super::resolve_range(&tree, Some(owner_range), &diagnostic);
        assert_eq!(range, TextRange::empty(owner_range.start()));
    }
}
