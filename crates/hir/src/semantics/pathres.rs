use smallvec::SmallVec;
use syntax::{SyntaxNode, SyntaxTokenWithParent};
use triomphe::Arc;
use utils::get::GetRef;

use super::SemanticsImpl;
use crate::{
    container::{ArenaOwnerId, InContainer, InFile, ScopeId, ScopeParent},
    db::HirDb,
    def_id::DefId,
    file::HirFileId,
    hir_def::{
        Ident, lower_ident_opt,
        module::{ModuleId, instantiation::InstanceId},
    },
    symbol::{DefKind, NameContext, NameScope, Resolution},
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

impl SemanticsImpl<'_> {
    pub fn nameres_ident(
        &self,
        file_id: HirFileId,
        SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
        name_ctx: NameContext,
    ) -> Resolution<DefId> {
        let Some(ident) = lower_ident_opt(Some(tok)) else {
            return Resolution::Unresolved;
        };
        self.with_ctx(|source_ctx| {
            let container = source_ctx.find_container(InFile::new(file_id, parent));
            source_ctx.name_to_def(InContainer::new(container, ident), name_ctx)
        })
    }

    pub(in crate::semantics) fn find_container(&self, node: InFile<SyntaxNode>) -> ArenaOwnerId {
        self.with_ctx(|ctx| ctx.find_container(node))
    }

    pub fn resolve_name(
        &self,
        cont_id: ScopeId,
        ident: &Ident,
        ctx: NameContext,
    ) -> Resolution<DefId> {
        resolve_name(self.db, cont_id, ident, ctx)
    }
}

pub fn resolve_name(
    db: &dyn HirDb,
    cont_id: ScopeId,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    let scopes = ScopeParent::start_from(db, cont_id).collect::<SmallVec<[_; 4]>>();

    for id in &scopes {
        let resolution = name_scope(db, *id).lookup(ctx, ident);
        if !resolution.is_unresolved() {
            return resolution;
        }
    }

    // IEEE 1800-2017 keeps package imports distinct from ordinary lexical
    // declarations: visible declarations in the lexical chain win, then
    // package imports are considered, and `$unit` remains an explicit outer
    // scope. `NameContext` chooses the namespace bucket at every phase.
    let imported = resolve_imported_name(db, &scopes, ident, ctx);
    if !imported.is_unresolved() {
        return imported;
    }

    db.unit_scope().lookup(ctx, ident)
}

pub fn resolve_path(
    db: &dyn HirDb,
    cont_id: ScopeId,
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
    db: &dyn HirDb,
    cont_id: ScopeId,
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
        resolve_name(db, cont_id, ident, NameContext::Type)
            .candidates()
            .iter()
            .copied()
            .filter(|def_id| def_id.kind(db).is_instantiable_def()),
    )
}

pub(super) fn resolve_child_name(
    db: &dyn HirDb,
    parent: &Resolution<DefId>,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    parent.and_then(|def_id| {
        let Some(scope_id) = descend_scope(db, def_id) else {
            return Resolution::Unresolved;
        };
        name_scope(db, scope_id).lookup(ctx, ident)
    })
}

pub fn descend_scope(db: &dyn HirDb, def_id: DefId) -> Option<ScopeId> {
    let origin = def_id.primary_origin(db);
    match def_id.kind(db) {
        DefKind::Module | DefKind::Interface | DefKind::Program => {
            origin.as_module(db).map(Into::into)
        }
        DefKind::ClockingBlock => origin.as_clocking_block(db).map(Into::into),
        DefKind::Checker => origin.as_checker(db).map(ScopeId::Checker),
        DefKind::Covergroup => origin.as_covergroup(db).map(ScopeId::Covergroup),
        DefKind::Instance => {
            let instance = origin.as_instance(db)?;
            let target = instance_target_def_id(db, instance.module_id, instance.value)?;
            descend_scope(db, target)
        }
        DefKind::Block => origin.as_block(db).map(Into::into),
        DefKind::GenerateBlock => origin.as_generate_block(db).map(Into::into),
        _ => None,
    }
}

pub(crate) fn instance_target_def_id(
    db: &dyn HirDb,
    module_id: ModuleId,
    instance_id: InstanceId,
) -> Option<DefId> {
    let module = db.module(module_id);
    let instance = module.get(instance_id);
    let instantiation = module.get(instance.parent);
    let module_name = instantiation.module_name.as_ref()?;
    let target = resolve_name(db, module_id.into(), module_name, NameContext::Type).unique()?;
    target.kind(db).is_instantiable_def().then_some(target)
}

pub(crate) fn name_scope(db: &dyn HirDb, scope_id: ScopeId) -> Arc<NameScope> {
    match scope_id {
        ScopeId::File(file_id) => db.file_scope(file_id),
        ScopeId::Module(module_id) => db.module_scope(module_id),
        ScopeId::ClockingBlock(clocking_block_id) => db.clocking_block_scope(clocking_block_id),
        ScopeId::Checker(checker_id) => db.checker_scope(checker_id),
        ScopeId::Covergroup(covergroup_id) => db.covergroup_scope(covergroup_id),
        ScopeId::GenerateBlock(generate_block_id) => db.generate_block_scope(generate_block_id),
        ScopeId::Block(block_id) => db.block_scope(block_id),
        ScopeId::Subroutine(subroutine_id) => db.subroutine_scope(subroutine_id),
    }
}

fn resolve_imported_name(
    db: &dyn HirDb,
    scopes: &[ScopeId],
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    let mut defs = SmallVec::<[DefId; 3]>::new();

    for scope_id in scopes {
        let scope = name_scope(db, *scope_id);
        collect_imports(db, &scope, ident, ctx, true, &mut defs);
        if !defs.is_empty() {
            return Resolution::from_candidates(defs);
        }
    }

    for scope_id in scopes {
        let scope = name_scope(db, *scope_id);
        collect_imports(db, &scope, ident, ctx, false, &mut defs);
        if !defs.is_empty() {
            return Resolution::from_candidates(defs);
        }
    }

    Resolution::Unresolved
}

fn collect_imports(
    db: &dyn HirDb,
    scope: &NameScope,
    ident: &Ident,
    ctx: NameContext,
    named_only: bool,
    defs: &mut SmallVec<[DefId; 3]>,
) {
    for import in &scope.imports {
        match (&import.name, named_only) {
            (Some(name), true) if name == ident => {}
            (None, false) => {}
            _ => continue,
        }

        let imported = db
            .unit_scope()
            .package_ids(db, &import.package)
            .and_then(|package_id| db.package_export_scope(package_id).lookup(ctx, ident));
        for def_id in imported.into_candidates() {
            if !defs.contains(&def_id) {
                defs.push(def_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use rustc_hash::FxHashSet;
    use smol_str::SmolStr;
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{
        base_db::{
            diagnostics_config::DiagnosticsConfig,
            project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
            salsa::{self, Durability},
            source_db::{
                FileLoader, SourceDb, SourceDbStorage, SourceFileKind, SourceRootDb,
                SourceRootDbStorage,
            },
            source_root::{SourceRoot, SourceRootId},
        },
        container::ScopeId,
        db::{HirDb, HirDbStorage, InternDbStorage},
        file::HirFileId,
        hir_def::Ident,
        symbol::{DefKind, NameContext},
    };

    const TOP: FileId = FileId::from_raw(0);
    const ROOT: SourceRootId = SourceRootId(0);
    const PROFILE: CompilationProfileId = CompilationProfileId(0);

    #[salsa::database(SourceDbStorage, SourceRootDbStorage, InternDbStorage, HirDbStorage)]
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

    fn ident(name: &str) -> Ident {
        SmolStr::new(name)
    }

    fn path(segments: &[&str]) -> Vec<Ident> {
        segments.iter().map(|segment| ident(segment)).collect()
    }

    fn resolved_kind(
        db: &TestDb,
        scope_id: ScopeId,
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
    end
  endgenerate
endmodule
"#,
        );

        let top = db
            .unit_scope()
            .module_ids(&db, &ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert_eq!(resolved_kind(&db, top.into(), &["u", "sig"], NameContext::Value), DefKind::Net);
        assert_eq!(
            resolved_kind(&db, top.into(), &["arr", "sig"], NameContext::Value),
            DefKind::Net
        );
        assert_eq!(
            resolved_kind(&db, top.into(), &["b", "local_sig"], NameContext::Value),
            DefKind::Variable
        );
        assert_eq!(
            resolved_kind(&db, top.into(), &["g", "gen_sig"], NameContext::Value),
            DefKind::Net
        );
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
            .unit_scope()
            .module_ids(&db, &ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert!(
            resolve_path(&db, top.into(), &path(&["u", "only_left"]), NameContext::Value)
                .is_unresolved()
        );
        let Resolution::Ambiguous(shared) =
            resolve_path(&db, top.into(), &path(&["u", "shared"]), NameContext::Value)
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
            .unit_scope()
            .module_ids(&db, &ident("top"))
            .unique()
            .expect("top module should resolve uniquely");
        let Resolution::Ambiguous(values) =
            resolve_name(&db, top.into(), &ident("value"), NameContext::Value)
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
            .unit_scope()
            .module_ids(&db, &ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert!(
            resolve_name(&db, top.into(), &ident("only_left"), NameContext::Value).is_unresolved(),
            "a child member must not disambiguate its parent package"
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
                ScopeId::File(HirFileId::File(TOP)),
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
            .unit_scope()
            .module_ids(&db, &ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        let res = resolve_path(&db, top.into(), &path(&["u_if", "host"]), NameContext::Value);

        let def = res.unique().expect("modport should produce a unique definition");
        assert_eq!(def.name(&db).as_deref(), Some("host"));
        assert_eq!(def.kind(&db), DefKind::Modport);
        assert_eq!(
            resolved_kind(&db, top.into(), &["u_if", "clk"], NameContext::Value),
            DefKind::Net
        );
        assert_eq!(
            resolved_kind(
                &db,
                ScopeId::File(HirFileId::File(TOP)),
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
            .unit_scope()
            .module_ids(&db, &ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert_eq!(
            resolved_kind(&db, top.into(), &["cb", "a"], NameContext::Value),
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
            .unit_scope()
            .module_ids(&db, &ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert_eq!(
            resolved_kind(&db, top.into(), &["u", "clk"], NameContext::Value),
            DefKind::CheckerPort
        );
        assert_eq!(
            resolved_kind(&db, top.into(), &["u", "sig"], NameContext::Value),
            DefKind::Variable
        );
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
            .unit_scope()
            .module_ids(&db, &ident("top"))
            .unique()
            .expect("top module should resolve uniquely");

        assert_eq!(
            resolved_kind(&db, top.into(), &["u", "cp"], NameContext::Value),
            DefKind::Coverpoint
        );
        assert_eq!(
            resolved_kind(&db, top.into(), &["u", "cx"], NameContext::Value),
            DefKind::Cross
        );
    }
}
