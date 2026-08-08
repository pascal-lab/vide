use smallvec::SmallVec;
use utils::get::GetRef;

use crate::{
    Ident,
    container::ScopeChain,
    db::HirDefDb,
    def_id::DefId,
    module::instantiation::InstanceId,
    owner::{OwnerId, OwnerKind},
    symbol::{DefKind, NameContext, Resolution, ScopeData},
};

// SystemVerilog name AST note for path resolution:
//
// slang models simple names as `IdentifierName`, names with unpacked selects
// as `IdentifierSelectName { identifier, selectors }`, and qualified names as
// `ScopedName { left, separator, right }`. The `separator` token is the only
// raw-AST distinction between `a.b` hierarchical selection and `a::b`
// package/class scoping. HIR lowering turns dot-style member access and
// `ScopedName` with an identifier right side into `Expr::Field`, and
// `IdentifierSelectName` into `Expr::ElementSelect`; C3's `resolve_path`
// handles the hierarchical dot/select shape only. Package/class `::` remains
// outside this resolver until those constructs are lowered.

/// Resolution phase recorded by [`resolve_name_with_trace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionPhase {
    Lexical,
    NamedImport,
    WildcardImport,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolutionTraceEntry {
    pub phase: ResolutionPhase,
    pub scope: Option<OwnerId>,
    pub resolution: Resolution<DefId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ResolutionTrace {
    entries: Vec<ResolutionTraceEntry>,
}

impl ResolutionTrace {
    pub fn entries(&self) -> &[ResolutionTraceEntry] {
        &self.entries
    }
}

pub fn resolve_name(
    db: &dyn HirDefDb,
    cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    resolve_name_inner(db, cont_id, ident, ctx, None)
}

/// Resolve a name and retain the precedence decisions made by the resolver.
///
/// The normal [`resolve_name`] path remains allocation-free for trace data.
/// Callers that need diagnostics or debugging can inspect each lexical,
/// named-import, wildcard-import, and `$unit` decision through this seam.
pub fn resolve_name_with_trace(
    db: &dyn HirDefDb,
    cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
) -> (Resolution<DefId>, ResolutionTrace) {
    let mut trace = ResolutionTrace::default();
    let resolution = resolve_name_inner(db, cont_id, ident, ctx, Some(&mut trace));
    (resolution, trace)
}

fn resolve_name_inner(
    db: &dyn HirDefDb,
    cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
    mut trace: Option<&mut ResolutionTrace>,
) -> Resolution<DefId> {
    let scopes = ScopeChain::from_inner(db, cont_id);
    for id in scopes.iter() {
        let resolution = db.scope(*id).lookup(ctx, ident);
        if let Some(trace) = trace.as_deref_mut() {
            trace.entries.push(ResolutionTraceEntry {
                phase: ResolutionPhase::Lexical,
                scope: Some(*id),
                resolution: resolution.clone(),
            });
        }
        if !resolution.is_unresolved() {
            return resolution;
        }
    }

    // Lexical declarations win over package imports; `$unit` remains the
    // explicit outer scope after both named and wildcard imports.
    let imported = resolve_imported_name(db, &scopes, ident, ctx, trace.as_deref_mut());
    if !imported.is_unresolved() {
        return imported;
    }

    let unit = db.unit_scope().lookup(ctx, ident);
    if let Some(trace) = trace {
        trace.entries.push(ResolutionTraceEntry {
            phase: ResolutionPhase::Unit,
            scope: None,
            resolution: unit.clone(),
        });
    }
    unit
}

/// A scope chain resolved against canonical owner-local scope queries.
pub struct ResolvedScopes {
    scope_chain: ScopeChain,
}

impl ResolvedScopes {
    pub fn new(_db: &dyn HirDefDb, scope_chain: ScopeChain) -> Self {
        Self { scope_chain }
    }
}

/// Resolves `ident` in a pre-resolved scope chain. Package import resolution
/// is deferred to the full resolver.
pub fn resolve_in_resolved_scopes(
    db: &dyn HirDefDb,
    resolved: &ResolvedScopes,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    for scope_id in resolved.scope_chain.iter() {
        let scope = db.scope(*scope_id);
        if scope.has_imports() {
            return resolve_name(db, *scope_id, ident, ctx);
        }
        let resolution = scope.lookup(ctx, ident);
        if !resolution.is_unresolved() {
            return resolution;
        }
    }
    db.unit_scope().lookup(ctx, ident)
}

pub fn resolve_path(
    db: &dyn HirDefDb,
    cont_id: OwnerId,
    path: &[Ident],
    ctx: NameContext,
) -> Resolution<DefId> {
    let Some((first, rest)) = path.split_first() else {
        return Resolution::Unresolved;
    };
    let mut current = resolve_name(db, cont_id, first, ctx)
        .or_else(|| resolve_top_level_module_root(db, cont_id, first, ctx, !rest.is_empty()));

    for (idx, segment) in rest.iter().enumerate() {
        let segment_ctx = if idx + 1 == rest.len() { ctx } else { NameContext::Value };
        current = resolve_child_name(db, &current, segment, segment_ctx);
        if current.is_unresolved() {
            break;
        }
    }

    current
}

fn resolve_top_level_module_root(
    db: &dyn HirDefDb,
    _cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
    has_child_segment: bool,
) -> Resolution<DefId> {
    if !has_child_segment || ctx != NameContext::Value {
        return Resolution::Unresolved;
    }

    // IEEE 1800 hierarchical names can start at a top-level module instance.
    // Vide has module definitions in the type namespace and no separate
    // elaborated top-instance DefId yet, so a multi-segment value path may use
    // a module definition as an explicit hierarchy root. This is not a single
    // segment value fallback: `top` alone remains a type-space module name.
    Resolution::from_candidates(
        db.unit_index()
            .module_ids(ident)
            .into_candidates()
            .into_iter()
            .map(|owner| DefId::from_source(db, crate::symbol::DefOriginLoc::Module(owner))),
    )
}

pub fn resolve_child_name(
    db: &dyn HirDefDb,
    parent: &Resolution<DefId>,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    parent.and_then(|def_id| {
        let Some(scope_id) = descend_scope(db, def_id) else {
            return Resolution::Unresolved;
        };
        db.scope(scope_id).lookup(ctx, ident)
    })
}
pub fn descend_scope(db: &dyn HirDefDb, def_id: DefId) -> Option<OwnerId> {
    let origin = def_id.primary_origin(db);
    match def_id.kind(db) {
        DefKind::Module | DefKind::Interface | DefKind::Program | DefKind::Package => {
            origin.as_module(db)
        }
        DefKind::ClockingBlock
        | DefKind::Checker
        | DefKind::Covergroup
        | DefKind::Block
        | DefKind::GenerateBlock => Some(definition_scope_owner(db, origin)),
        DefKind::Instance => {
            let instance = origin.as_instance(db)?;
            let target = instance_target_def_id(db, instance.cont_id, instance.value)?;
            descend_scope(db, target)
        }
        _ => None,
    }
}

fn definition_scope_owner(db: &dyn HirDefDb, origin: crate::symbol::DefOrigin) -> OwnerId {
    origin.loc(db).clone().owner(db)
}

pub fn instance_target_def_id(
    db: &dyn HirDefDb,
    module_id: OwnerId,
    instance_id: InstanceId,
) -> Option<DefId> {
    let module = db.body(module_id);
    let instance = module.get(instance_id);
    let instantiation = module.get(instance.parent);
    let module_name = instantiation.module_name.as_ref()?;
    let target = db
        .unit_index()
        .instantiable_ids_in(module_id, module_name)
        .unique()
        .map(|owner| instantiable_def_id(db, owner))?;
    Some(target)
}

fn instantiable_def_id(db: &dyn HirDefDb, owner: OwnerId) -> DefId {
    let is_instantiable = matches!(owner.kind(db), OwnerKind::Checker | OwnerKind::Covergroup)
        || owner.module_kind(db).is_some_and(|kind| kind.is_instantiable());
    assert!(is_instantiable, "owner must be an instantiable design unit: {owner:?}");
    DefId::from_owner(db, owner).expect("instantiable owner must have a definition")
}
fn resolve_imported_name(
    db: &dyn HirDefDb,
    scopes: &ScopeChain,
    ident: &Ident,
    ctx: NameContext,
    mut trace: Option<&mut ResolutionTrace>,
) -> Resolution<DefId> {
    let design_map = db.design_map();
    let mut defs = SmallVec::<[DefId; 3]>::new();

    for scope_id in scopes.iter() {
        let scope = db.scope(*scope_id);
        collect_imports(db, &design_map, scope.as_ref(), ident, ctx, true, &mut defs);
        let resolution = Resolution::from_candidates(defs.iter().copied());
        if let Some(trace) = trace.as_deref_mut() {
            trace.entries.push(ResolutionTraceEntry {
                phase: ResolutionPhase::NamedImport,
                scope: Some(*scope_id),
                resolution: resolution.clone(),
            });
        }
        if !defs.is_empty() {
            return resolution;
        }
    }

    for scope_id in scopes.iter() {
        let scope = db.scope(*scope_id);
        collect_imports(db, &design_map, scope.as_ref(), ident, ctx, false, &mut defs);
        let resolution = Resolution::from_candidates(defs.iter().copied());
        if let Some(trace) = trace.as_deref_mut() {
            trace.entries.push(ResolutionTraceEntry {
                phase: ResolutionPhase::WildcardImport,
                scope: Some(*scope_id),
                resolution: resolution.clone(),
            });
        }
        if !defs.is_empty() {
            return resolution;
        }
    }

    Resolution::Unresolved
}

fn collect_imports(
    db: &dyn HirDefDb,
    design_map: &crate::design_map::DesignMap,
    scope: &ScopeData,
    ident: &Ident,
    ctx: NameContext,
    named_only: bool,
    defs: &mut SmallVec<[DefId; 3]>,
) {
    for import in scope.imports() {
        if named_only != import.name.is_some() {
            continue;
        }
        for def_id in design_map.resolve_import(db, import, ident, ctx).into_candidates() {
            if !defs.contains(&def_id) {
                defs.push(def_id);
            }
        }
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
    use smol_str::SmolStr;
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{
        Ident,
        db::HirDefDb,
        owner::OwnerId,
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
        SmolStr::new(name)
    }

    fn path(segments: &[&str]) -> Vec<Ident> {
        segments.iter().map(|segment| ident(segment)).collect()
    }

    fn resolved_kind(
        db: &TestDb,
        scope_id: OwnerId,
        segments: &[&str],
        ctx: NameContext,
    ) -> DefKind {
        let path = path(segments);
        resolve_path(db, scope_id, &path, ctx)
            .unique()
            .map(|def_id| def_id.kind(db))
            .unwrap_or_else(|| panic!("path {segments:?} should resolve"))
    }

    #[test]
    fn resolve_path_descends_instances_blocks_and_generate_blocks() {
        let db = db_with_root_text(
            r#"
module child;
  wire sig;
endmodule

module top;
  child u();
  child arr [1:0] ();

  initial begin : b
    integer local_sig;
  end

  generate
    if (1) begin : g
      wire gen_sig;
      child gen_u();
    end
  endgenerate
endmodule
"#,
        );

        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert_eq!(resolved_kind(&db, top, &["u", "sig"], NameContext::Value), DefKind::Net);
        assert_eq!(resolved_kind(&db, top, &["arr", "sig"], NameContext::Value), DefKind::Net);
        assert_eq!(
            resolved_kind(&db, top, &["b", "local_sig"], NameContext::Value),
            DefKind::Variable
        );
        assert_eq!(resolved_kind(&db, top, &["g", "gen_sig"], NameContext::Value), DefKind::Net);
        assert_eq!(resolved_kind(&db, top, &["g", "gen_u"], NameContext::Value), DefKind::Instance);
    }

    #[test]
    fn resolve_path_does_not_collapse_ambiguous_parent() {
        let db = db_with_root_text(
            r#"
module left;
  wire only_left;
  wire shared;
endmodule

module right;
  wire shared;
endmodule

module top;
  left u();
  right u();
endmodule
"#,
        );
        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert!(
            resolve_path(&db, top, &path(&["u", "only_left"]), NameContext::Value).is_unresolved()
        );
        let Resolution::Ambiguous(shared) =
            resolve_path(&db, top, &path(&["u", "shared"]), NameContext::Value)
        else {
            panic!("members from ambiguous parents should remain ambiguous");
        };
        assert_eq!(shared.len(), 2);
    }

    #[test]
    fn wildcard_import_preserves_ambiguous_packages() {
        let db = db_with_root_text(
            r#"
package p;
  int value;
endpackage

package p;
  int value;
endpackage

module top;
  import p::*;
endmodule
"#,
        );
        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");
        let Resolution::Ambiguous(values) =
            resolve_name(&db, top, &ident("value"), NameContext::Value)
        else {
            panic!("imports from ambiguous packages should remain ambiguous");
        };
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn wildcard_import_does_not_resolve_through_one_ambiguous_package() {
        let db = db_with_root_text(
            r#"
package p;
  int only_left;
endpackage

package p;
endpackage

module top;
  import p::*;
endmodule
"#,
        );
        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert!(
            resolve_name(&db, top, &ident("only_left"), NameContext::Value).is_unresolved(),
            "a child member must not disambiguate its parent package"
        );
    }

    #[test]
    fn resolution_trace_records_named_import_precedence() {
        let db = db_with_root_text(
            r#"
package wildcard;
  int value;
endpackage

package named;
  int value;
endpackage

module top;
  import wildcard::*;
  import named::value;
endmodule
"#,
        );
        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");
        let named = db
            .unit_index()
            .package_ids(&ident("named"))
            .unique()
            .expect("named package should resolve uniquely");
        let expected = db
            .package_exports(named)
            .lookup(NameContext::Value, &ident("value"))
            .unique()
            .expect("named package value should resolve uniquely");

        let (resolved, trace) =
            resolve_name_with_trace(&db, top, &ident("value"), NameContext::Value);
        assert_eq!(resolved, Resolution::Unique(expected));
        assert!(trace.entries().iter().any(|entry| {
            entry.phase == ResolutionPhase::NamedImport
                && entry.resolution == Resolution::Unique(expected)
        }));
        assert!(
            !trace.entries().iter().any(|entry| {
                entry.phase == ResolutionPhase::WildcardImport && !entry.resolution.is_unresolved()
            }),
            "wildcard imports must not run after a named import resolves"
        );
    }

    #[test]
    fn named_imports_preserve_ambiguity() {
        let db = db_with_root_text(
            r#"
package left;
  int value;
endpackage

package right;
  int value;
endpackage

module top;
  import left::value;
  import right::value;
endmodule
"#,
        );
        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");
        let (resolved, trace) =
            resolve_name_with_trace(&db, top, &ident("value"), NameContext::Value);
        let Resolution::Ambiguous(candidates) = resolved else {
            panic!("two named imports must remain ambiguous");
        };
        assert_eq!(candidates.len(), 2);
        assert!(trace.entries().iter().any(|entry| {
            entry.phase == ResolutionPhase::NamedImport
                && matches!(entry.resolution, Resolution::Ambiguous(_))
        }));
    }

    #[test]
    fn nested_package_imports_reach_a_fixed_point() {
        let db = db_with_root_text(
            r#"
package outer;
  import middle::*;
endpackage

package middle;
  import base::*;
endpackage

package base;
  int value;
endpackage

module top;
  import outer::*;
endmodule
"#,
        );

        let outer = db
            .unit_index()
            .package_ids(&ident("outer"))
            .unique()
            .expect("outer package should resolve uniquely");
        assert!(
            db.package_exports(outer)
                .lookup(NameContext::Value, &ident("value"))
                .unique()
                .is_some(),
            "nested package exports must be computed transitively"
        );

        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");
        assert!(
            resolve_name(&db, top, &ident("value"), NameContext::Value).unique().is_some(),
            "lexical resolution must consume the canonical design map"
        );
    }

    #[test]
    fn resolve_path_treats_top_level_module_as_hierarchical_root() {
        let db = db_with_root_text(
            r#"
module child;
  wire sig;
endmodule

module top;
  child u();
endmodule
"#,
        );

        assert_eq!(
            resolved_kind(
                &db,
                db.owner_table(HirFileId::File(TOP)).file_owner().expect("file owner"),
                &["top", "u", "sig"],
                NameContext::Value,
            ),
            DefKind::Net
        );
    }

    #[test]
    fn resolve_path_descends_interface_instances_to_modports() {
        let db = db_with_root_text(
            r#"
interface bus_if;
  wire clk;
  modport host(input clk);
endinterface

module top;
  bus_if u_if();
endmodule
"#,
        );

        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        let res = resolve_path(&db, top, &path(&["u_if", "host"]), NameContext::Value);

        let def = res.unique().expect("modport should produce a unique definition");
        assert_eq!(def.name(&db).as_deref(), Some("host"));
        assert_eq!(def.kind(&db), DefKind::Modport);
        assert_eq!(resolved_kind(&db, top, &["u_if", "clk"], NameContext::Value), DefKind::Net);
        assert_eq!(
            resolved_kind(
                &db,
                db.owner_table(HirFileId::File(TOP)).file_owner().expect("file owner"),
                &["top", "u_if", "host"],
                NameContext::Value,
            ),
            DefKind::Modport
        );
    }

    #[test]
    fn resolve_path_descends_clocking_blocks_to_signals() {
        let db = db_with_root_text(
            r#"
module top(input clk, input a);
  clocking cb @(posedge clk);
    input #1ps a;
  endclocking
endmodule
"#,
        );

        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert_eq!(
            resolved_kind(&db, top, &["cb", "a"], NameContext::Value),
            DefKind::ClockingSignal
        );
    }

    #[test]
    fn resolve_path_descends_checker_instances_to_ports_and_members() {
        let db = db_with_root_text(
            r#"
checker c(input logic clk);
  logic sig;
endchecker

module top;
  c u();
endmodule
"#,
        );

        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert_eq!(
            resolved_kind(&db, top, &["u", "clk"], NameContext::Value),
            DefKind::CheckerPort
        );
        assert_eq!(resolved_kind(&db, top, &["u", "sig"], NameContext::Value), DefKind::Variable);
    }

    #[test]
    fn resolve_path_descends_covergroup_instances_to_coverage_items() {
        let db = db_with_root_text(
            r#"
module top(input clk, input a);
  covergroup cg @(posedge clk);
    cp: coverpoint a;
    cx: cross cp;
  endgroup

  cg u();
endmodule
"#,
        );

        let top = db
            .unit_index()
            .module_ids(&ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert_eq!(resolved_kind(&db, top, &["u", "cp"], NameContext::Value), DefKind::Coverpoint);
        assert_eq!(resolved_kind(&db, top, &["u", "cx"], NameContext::Value), DefKind::Cross);
    }
}
