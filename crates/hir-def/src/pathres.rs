use preproc_expand::file::HirFileId;
use smallvec::SmallVec;
use triomphe::Arc;
use utils::get::GetRef;

use crate::{
    Ident,
    container::{InFile, ScopeChain},
    db::HirDefDb,
    def_id::DefId,
    design_map::DesignMap,
    module::instantiation::InstanceId,
    owner::{OwnerId, OwnerKind},
    symbol::{DefKind, NameContext, Resolution, ScopeData},
    unit::ToOwner,
};

/// Cross-file name-resolution inputs.
///
/// The injected [`UnitCatalog`] answers compilation-unit names. `$unit`
/// locals and the package export map are paid when the context is built
/// from the current catalog, not stored as a third memo.
#[derive(Clone)]
pub struct ResolutionContext {
    graph: Arc<design_graph::UnitCatalog>,
    unit_scope: Arc<ScopeData>,
    design_map: Arc<DesignMap>,
}

impl ResolutionContext {
    pub fn from_graph(db: &dyn HirDefDb, graph: Arc<design_graph::UnitCatalog>) -> Arc<Self> {
        Arc::new(Self {
            unit_scope: db.unit_scope(),
            design_map: crate::design_map::package_export_closure(db, &graph),
            graph,
        })
    }

    pub fn graph(&self) -> &design_graph::UnitCatalog {
        &self.graph
    }

    pub fn unit_scope(&self, _db: &dyn HirDefDb) -> Arc<ScopeData> {
        self.unit_scope.clone()
    }

    pub fn design_map(&self, _db: &dyn HirDefDb) -> Arc<DesignMap> {
        self.design_map.clone()
    }
}

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

/// How a name reference is used; controls forward visibility per
/// (function/task calls search to the end of their scope).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    Value,
    Call,
}

/// A name reference with its source position. Passing one enables the
/// point-of-reference rules: declarations and explicit imports count only
/// before the reference in the innermost scope, and wildcard imports count
/// only before the reference in every scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NameRef {
    pub position: InFile<crate::ast_id_map::SourceAstId>,
    pub kind: RefKind,
}

pub fn resolve_name(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    resolve_name_at(db, context, cont_id, ident, ctx, None)
}

/// Resolve a name honoring the reference's source position. Without a
/// reference every declaration and import in a scope is considered, which
/// matches the position-less [`resolve_name`].
pub fn resolve_name_at(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
    reference: Option<&NameRef>,
) -> Resolution<DefId> {
    resolve_name_inner(db, context, cont_id, ident, ctx, None, reference)
}

/// Resolve a name and retain the precedence decisions made by the resolver.
///
/// The normal [`resolve_name`] path remains allocation-free for trace data.
/// Callers that need diagnostics or debugging can inspect each lexical,
/// named-import, wildcard-import, and `$unit` decision through this seam.
pub fn resolve_name_with_trace(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
) -> (Resolution<DefId>, ResolutionTrace) {
    let mut trace = ResolutionTrace::default();
    let resolution = resolve_name_inner(db, context, cont_id, ident, ctx, Some(&mut trace), None);
    (resolution, trace)
}

/// Whether a source position precedes a reference position. Cross-file
/// positions are unordered and therefore always visible (keeps multi-file
/// resolution unchanged); ordinals are compared within the shared file map.
pub(crate) fn before_reference(
    db: &dyn HirDefDb,
    source: InFile<crate::ast_id_map::SourceAstId>,
    reference: &NameRef,
) -> bool {
    if source.file_id != reference.position.file_id {
        return true;
    }
    let map = db.ast_id_map(source.file_id);
    match (map.preorder(source.value), map.preorder(reference.position.value)) {
        (Some(ordinal), Some(reference_ordinal)) => ordinal < reference_ordinal,
        _ => true,
    }
}

/// Keep only definitions whose source precedes the reference point.
fn filter_resolution_at(
    db: &dyn HirDefDb,
    resolution: Resolution<DefId>,
    reference: &NameRef,
) -> Resolution<DefId> {
    Resolution::from_candidates(resolution.into_candidates().into_iter().filter(|def_id| {
        match def_id.primary_origin(db).loc(db).clone().source_ast(db) {
            Some(source) => before_reference(db, source, reference),
            None => true,
        }
    }))
}

fn resolve_name_inner(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
    mut trace: Option<&mut ResolutionTrace>,
    reference: Option<&NameRef>,
) -> Resolution<DefId> {
    let scopes = ScopeChain::from_inner(db, cont_id);
    let is_call = reference.is_some_and(|reference| reference.kind == RefKind::Call);
    for id in scopes.iter() {
        let scope = db.scope(*id);
        let resolution = scope.lookup(ctx, ident);
        // Declarations are visible only before the reference in every scope,
        // unless the reference is a function/task call, which searches each
        // scope to its end (IEEE 1800-2017 26.3).
        let resolution = match (is_call, reference) {
            (false, Some(reference)) => filter_resolution_at(db, resolution, reference),
            _ => resolution,
        };
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

        // Each scope is searched completely before the next outer scope:
        // declarations and explicit imports first, then wildcard imports of
        // this scope. `$unit` remains the final scope.
        let imported = resolve_scope_imports(
            db,
            context,
            scope.as_ref(),
            ident,
            ctx,
            *id,
            trace.as_deref_mut(),
            AtFilter { reference },
        );
        if !imported.is_unresolved() {
            return imported;
        }
    }

    let unit = resolve_unit_name(db, context, ident, ctx);
    if let Some(trace) = trace {
        trace.entries.push(ResolutionTraceEntry {
            phase: ResolutionPhase::Unit,
            scope: None,
            resolution: unit.clone(),
        });
    }
    unit
}

fn resolve_unit_name(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    let locals = context.unit_scope(db).lookup(ctx, ident);
    let units = match ctx {
        NameContext::Type | NameContext::Listing => Resolution::from_candidates(
            context.graph().type_units_named(ident).into_vec().into_iter().filter_map(|unit| {
                unit.to_owner(db).and_then(|owner| DefId::from_owner(db, owner))
            }),
        ),
        NameContext::Value | NameContext::Assertion => Resolution::Unresolved,
    };
    match (locals, units) {
        (Resolution::Unresolved, other) | (other, Resolution::Unresolved) => other,
        (left, right) => Resolution::from_candidates(
            left.into_candidates().into_iter().chain(right.into_candidates()),
        ),
    }
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

/// Resolves `ident` in a pre-resolved scope chain using the same per-scope
/// search order as [`resolve_name_at`].
pub fn resolve_in_resolved_scopes(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    resolved: &ResolvedScopes,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    resolve_in_resolved_scopes_at(db, context, resolved, ident, ctx, None)
}

/// Position-aware variant of [`resolve_in_resolved_scopes`]; see
/// [`resolve_name_at`] for the filtering rules.
pub fn resolve_in_resolved_scopes_at(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    resolved: &ResolvedScopes,
    ident: &Ident,
    ctx: NameContext,
    reference: Option<&NameRef>,
) -> Resolution<DefId> {
    let is_call = reference.is_some_and(|reference| reference.kind == RefKind::Call);
    for scope_id in resolved.scope_chain.iter() {
        let scope = db.scope(*scope_id);
        let resolution = scope.lookup(ctx, ident);
        let resolution = match (is_call, reference) {
            (false, Some(reference)) => filter_resolution_at(db, resolution, reference),
            _ => resolution,
        };
        if !resolution.is_unresolved() {
            return resolution;
        }
        let imported = resolve_scope_imports(
            db,
            context,
            scope.as_ref(),
            ident,
            ctx,
            *scope_id,
            None,
            AtFilter { reference },
        );
        if !imported.is_unresolved() {
            return imported;
        }
    }
    resolve_unit_name(db, context, ident, ctx)
}

pub fn resolve_path(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    cont_id: OwnerId,
    path: &[Ident],
    ctx: NameContext,
) -> Resolution<DefId> {
    resolve_path_at(db, context, cont_id, path, ctx, None)
}

/// Position-aware variant of [`resolve_path`]; the first segment honors the
/// reference position while member segments keep position-less lookup.
pub fn resolve_path_at(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    cont_id: OwnerId,
    path: &[Ident],
    ctx: NameContext,
    reference: Option<&NameRef>,
) -> Resolution<DefId> {
    let Some((first, rest)) = path.split_first() else {
        return Resolution::Unresolved;
    };
    let mut current = resolve_name_at(db, context, cont_id, first, ctx, reference)
        .or_else(|| resolve_top_level_module_root(db, context, first, ctx, !rest.is_empty()));

    for (idx, segment) in rest.iter().enumerate() {
        let segment_ctx = if idx + 1 == rest.len() { ctx } else { NameContext::Value };
        current = resolve_child_name(db, context, &current, segment, segment_ctx);
        if current.is_unresolved() {
            break;
        }
    }

    current
}

fn resolve_top_level_module_root(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
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
    // a compilation-unit module definition as an explicit hierarchy root. This
    // is not a single segment value fallback: `top` alone remains a type-space
    // module name, and nested declarations never leak through the fallback.
    Resolution::from_candidates(
        context.graph().modules_named(ident).into_vec().into_iter().filter_map(|unit| {
            unit.to_owner(db)
                .map(|owner| DefId::from_source(db, crate::symbol::DefOriginLoc::Module(owner)))
        }),
    )
}

pub fn resolve_child_name(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    parent: &Resolution<DefId>,
    ident: &Ident,
    ctx: NameContext,
) -> Resolution<DefId> {
    parent.and_then(|def_id| {
        let Some(scope_id) = descend_scope(db, context, def_id) else {
            return Resolution::Unresolved;
        };
        db.scope(scope_id).lookup(ctx, ident)
    })
}
pub fn descend_scope(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    def_id: DefId,
) -> Option<OwnerId> {
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
            let target = instance_target_def_id(db, context, instance.cont_id, instance.value)?;
            descend_scope(db, context, target)
        }
        _ => None,
    }
}

fn definition_scope_owner(db: &dyn HirDefDb, origin: crate::symbol::DefOrigin) -> OwnerId {
    origin.loc(db).clone().owner(db)
}

pub fn instance_target_def_id(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    module_id: OwnerId,
    instance_id: InstanceId,
) -> Option<DefId> {
    let module = db.body(module_id);
    let instance = module.get(instance_id);
    let instantiation = module.get(instance.parent);
    let module_name = instantiation.module_name.as_ref()?;
    let local = local_instantiable_owner(db, module_id, module_name);
    if !local.is_unresolved() {
        return local.unique().map(|owner| instantiable_def_id(db, owner));
    }
    let target = Resolution::from_candidates(
        context
            .graph()
            .candidates(module_name, design_graph::InstantiationRole::Hierarchy)
            .into_iter()
            .chain(
                context.graph().candidates(module_name, design_graph::InstantiationRole::Checker),
            )
            .filter_map(|unit| unit.to_owner(db)),
    )
    .unique()
    .map(|owner| instantiable_def_id(db, owner))?;
    Some(target)
}

fn local_instantiable_owner(
    db: &dyn HirDefDb,
    scope: OwnerId,
    name: &Ident,
) -> Resolution<OwnerId> {
    Resolution::from_candidates(
        db.owner_table(scope.file(db))
            .owners()
            .iter()
            .filter(|owner| {
                owner.parent == Some(scope)
                    && owner.name == *name
                    && matches!(owner.kind, OwnerKind::Checker | OwnerKind::Covergroup)
            })
            .map(|owner| owner.id),
    )
}

fn instantiable_def_id(db: &dyn HirDefDb, owner: OwnerId) -> DefId {
    let is_instantiable = matches!(owner.kind(db), OwnerKind::Checker | OwnerKind::Covergroup)
        || owner.module_kind(db).is_some_and(|kind| kind.is_instantiable());
    assert!(is_instantiable, "owner must be an instantiable design unit: {owner:?}");
    DefId::from_owner(db, owner).expect("instantiable owner must have a definition")
}
/// Point-of-reference filter for one name lookup (IEEE 1800-2017 26.3).
#[derive(Clone, Copy)]
struct AtFilter<'a> {
    reference: Option<&'a NameRef>,
}

impl AtFilter<'_> {
    fn is_call(&self) -> bool {
        self.reference.is_some_and(|reference| reference.kind == RefKind::Call)
    }

    /// Declarations and explicit imports are locally visible only before the
    /// reference in every scope; function/task call references search every
    /// scope to its end (IEEE 1800-2017 26.3, Examples 1/3).
    fn filter_named(&self) -> bool {
        !self.is_call() && self.reference.is_some()
    }

    /// Wildcard imports count only before the reference in every scope.
    fn filter_wildcard(&self) -> bool {
        self.reference.is_some()
    }
}

/// Collects import candidates for one scope, applying the point filter.
struct ImportCollector<'a> {
    db: &'a dyn HirDefDb,
    graph: &'a design_graph::UnitCatalog,
    design_map: &'a crate::design_map::DesignMap,
    scope: &'a ScopeData,
    defs: SmallVec<[DefId; 3]>,
    scope_file: HirFileId,
    at: AtFilter<'a>,
}

impl ImportCollector<'_> {
    fn collect(&mut self, ident: &Ident, ctx: NameContext, named_only: bool) {
        let filter = if named_only { self.at.filter_named() } else { self.at.filter_wildcard() };
        for import in self.scope.imports() {
            if named_only != import.name.is_some() {
                continue;
            }
            if filter
                && let (Some(reference), Some(source)) = (self.at.reference, import.source)
                && !before_reference(self.db, InFile::new(self.scope_file, source), reference)
            {
                continue;
            }
            for def_id in self
                .design_map
                .resolve_import(self.db, self.graph, import, ident, ctx)
                .into_candidates()
            {
                if !self.defs.contains(&def_id) {
                    self.defs.push(def_id);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_scope_imports(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    scope: &ScopeData,
    ident: &Ident,
    ctx: NameContext,
    scope_id: OwnerId,
    mut trace: Option<&mut ResolutionTrace>,
    at: AtFilter<'_>,
) -> Resolution<DefId> {
    let design_map = context.design_map(db);
    let mut collector = ImportCollector {
        db,
        graph: context.graph(),
        design_map: design_map.as_ref(),
        scope,
        defs: SmallVec::new(),
        scope_file: scope_id.file(db),
        at,
    };

    collector.collect(ident, ctx, true);
    let named = Resolution::from_candidates(collector.defs.iter().copied());
    if let Some(trace) = trace.as_mut() {
        trace.entries.push(ResolutionTraceEntry {
            phase: ResolutionPhase::NamedImport,
            scope: Some(scope_id),
            resolution: named.clone(),
        });
    }
    if !named.is_unresolved() {
        return named;
    }

    collector.collect(ident, ctx, false);
    let wildcard = Resolution::from_candidates(collector.defs.iter().copied());
    if let Some(trace) = trace.as_mut() {
        trace.entries.push(ResolutionTraceEntry {
            phase: ResolutionPhase::WildcardImport,
            scope: Some(scope_id),
            resolution: wildcard.clone(),
        });
    }
    wildcard
}

/// Resolves `ident` through wildcard imports only, returning the scope whose
/// wildcard import matched. Used to detect when a reference makes a wildcard
/// import locally visible (IEEE 1800-2017 26.3).
pub(crate) fn resolve_wildcard_at(
    db: &dyn HirDefDb,
    context: &ResolutionContext,
    cont_id: OwnerId,
    ident: &Ident,
    ctx: NameContext,
    reference: Option<&NameRef>,
) -> (Resolution<DefId>, Option<OwnerId>) {
    let scopes = ScopeChain::from_inner(db, cont_id);
    let at = AtFilter { reference };
    for scope_id in scopes.iter() {
        let scope = db.scope(*scope_id);
        let design_map = context.design_map(db);
        let mut collector = ImportCollector {
            db,
            graph: context.graph(),
            design_map: design_map.as_ref(),
            scope: scope.as_ref(),
            defs: SmallVec::new(),
            scope_file: scope_id.file(db),
            at,
        };
        collector.collect(ident, ctx, false);
        let wildcard = Resolution::from_candidates(collector.defs.iter().copied());
        if !wildcard.is_unresolved() {
            return (wildcard, Some(*scope_id));
        }
    }
    (Resolution::Unresolved, None)
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
    use syntax::has_text_range::HasTextRange;
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{
        Ident,
        container::InFile,
        db::HirDefDb,
        owner::{OwnerId, OwnerKind},
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
        resolve_path(db, &crate::unit::test_resolution(db), scope_id, &path, ctx)
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

        let top = crate::unit::test_module_owner(&db, "top");

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
        let top = crate::unit::test_module_owner(&db, "top");

        assert!(
            resolve_path(
                &db,
                &crate::unit::test_resolution(&db),
                top,
                &path(&["u", "only_left"]),
                NameContext::Value
            )
            .is_unresolved()
        );
        let Resolution::Ambiguous(shared) = resolve_path(
            &db,
            &crate::unit::test_resolution(&db),
            top,
            &path(&["u", "shared"]),
            NameContext::Value,
        ) else {
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
        let top = crate::unit::test_module_owner(&db, "top");
        let Resolution::Ambiguous(values) = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            top,
            &ident("value"),
            NameContext::Value,
        ) else {
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
        let top = crate::unit::test_module_owner(&db, "top");

        assert!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                top,
                &ident("only_left"),
                NameContext::Value
            )
            .is_unresolved(),
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
        let top = crate::unit::test_module_owner(&db, "top");
        let named = crate::unit::test_package_owner(&db, "named");
        let expected = db
            .package_exports(&crate::unit::test_resolution(&db), named)
            .lookup(NameContext::Value, &ident("value"))
            .unique()
            .expect("named package value should resolve uniquely");

        let (resolved, trace) = resolve_name_with_trace(
            &db,
            &crate::unit::test_resolution(&db),
            top,
            &ident("value"),
            NameContext::Value,
        );
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
        let top = crate::unit::test_module_owner(&db, "top");
        let (resolved, trace) = resolve_name_with_trace(
            &db,
            &crate::unit::test_resolution(&db),
            top,
            &ident("value"),
            NameContext::Value,
        );
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
    fn inner_scope_wildcard_shadows_outer_named_import() {
        // IEEE 1800-2023 26.3: each scope is searched completely (including
        // its wildcard imports) before the next outer scope, so the module
        // wildcard import wins over the compilation-unit named import.
        let db = db_with_root_text(
            r#"
import p::x;
package p;
int x;
endpackage
package p2;
int x;
endpackage
module top;
import p2::*;
initial x = 1;
endmodule
"#,
        );
        let top = crate::unit::test_module_owner(&db, "top");
        let p2 = crate::unit::test_package_owner(&db, "p2");
        let p2_x = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            p2,
            &ident("x"),
            NameContext::Value,
        )
        .unique()
        .expect("p2::x");
        assert_eq!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                top,
                &ident("x"),
                NameContext::Value
            ),
            Resolution::Unique(p2_x)
        );
    }

    #[test]
    fn generate_block_imports_are_locally_visible() {
        // An import inside a generate block is a member of that scope and is
        // searched before the enclosing module scope (IEEE 1800-2017 26.3).
        let db = db_with_root_text(
            r#"
package p;
int x;
endpackage
package p2;
int x;
endpackage
module top;
import p::*;
if (1) begin : b
import p2::*;
initial x = 1;
end
endmodule
"#,
        );
        let block = db
            .owner_table(HirFileId::File(TOP))
            .owners_of_kind(crate::owner::OwnerKind::GenerateBlock)
            .find(|owner| owner.name.as_str() == "b")
            .expect("generate block b owner")
            .id;
        let p2 = crate::unit::test_package_owner(&db, "p2");
        let p2_x = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            p2,
            &ident("x"),
            NameContext::Value,
        )
        .unique()
        .expect("p2::x");
        assert_eq!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                block,
                &ident("x"),
                NameContext::Value
            ),
            Resolution::Unique(p2_x)
        );
    }

    #[test]
    fn nested_package_imports_reach_a_fixed_point() {
        let db = db_with_root_text(
            r#"
package outer;
  import middle::*;
  export middle::*;
endpackage

package middle;
  import base::*;
  export base::*;
endpackage

package base;
  int value;
endpackage

module top;
  import outer::*;
endmodule
"#,
        );

        let outer = crate::unit::test_package_owner(&db, "outer");
        assert!(
            db.package_exports(&crate::unit::test_resolution(&db), outer)
                .lookup(NameContext::Value, &ident("value"))
                .unique()
                .is_some(),
            "nested package exports must be computed transitively"
        );

        let top = crate::unit::test_module_owner(&db, "top");
        assert!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                top,
                &ident("value"),
                NameContext::Value
            )
            .unique()
            .is_some(),
            "lexical resolution must consume the canonical design map"
        );
    }

    #[test]
    fn selective_and_export_all_package_exports_are_distinct() {
        let db = db_with_root_text(
            r#"
package base;
  int exported;
  int private;
endpackage
package selective;
  import base::*;
  export base::exported;
endpackage
package all;
  import base::*;
  export *::*;
endpackage
module top;
  import selective::*;
  import all::*;
endmodule
"#,
        );
        let top = crate::unit::test_module_owner(&db, "top");
        let selective = crate::unit::test_package_owner(&db, "selective");
        assert!(
            db.package_exports(&crate::unit::test_resolution(&db), selective)
                .lookup(NameContext::Value, &ident("exported"))
                .unique()
                .is_some(),
            "selective export must expose the selected imported value"
        );
        assert!(
            db.package_exports(&crate::unit::test_resolution(&db), selective)
                .lookup(NameContext::Value, &ident("private"))
                .is_unresolved(),
            "selective export must not expose other wildcard-imported values"
        );
        assert!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                top,
                &ident("private"),
                NameContext::Value
            )
            .unique()
            .is_some(),
            "export-all must re-export wildcard-imported values"
        );
    }

    #[test]
    fn mutually_importing_packages_reach_a_fixed_point() {
        let db = db_with_root_text(
            r#"
package p;
import q::*;
export q::*;
int x;
endpackage
package q;
import p::*;
export p::*;
int x;
endpackage
module top;
import p::*;
endmodule
"#,
        );
        let p = crate::unit::test_package_owner(&db, "p");
        let Resolution::Ambiguous(candidates) = db
            .package_exports(&crate::unit::test_resolution(&db), p)
            .lookup(NameContext::Value, &ident("x"))
        else {
            panic!("mutually exported x must remain ambiguous");
        };
        assert_eq!(candidates.len(), 2, "p::x and q::x must both be exported");

        let top = crate::unit::test_module_owner(&db, "top");
        let Resolution::Ambiguous(candidates) = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            top,
            &ident("x"),
            NameContext::Value,
        ) else {
            panic!("star import of mutually importing packages must stay ambiguous");
        };
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn star_import_closes_through_package_wildcards() {
        let db = db_with_root_text(
            r#"
package base;
int value;
endpackage
package middle;
import base::*;
export base::*;
endpackage
module top;
import middle::*;
endmodule
"#,
        );
        let base = crate::unit::test_package_owner(&db, "base");
        let expected = db
            .package_exports(&crate::unit::test_resolution(&db), base)
            .lookup(NameContext::Value, &ident("value"))
            .unique()
            .expect("base::value");
        let top = crate::unit::test_module_owner(&db, "top");
        assert_eq!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                top,
                &ident("value"),
                NameContext::Value
            ),
            Resolution::Unique(expected)
        );
    }

    #[test]
    fn def_id_survives_inserted_sibling_declaration() {
        let mut db = db_with_root_text("module m;\nint b;\nendmodule\n");
        let module_id = crate::unit::test_module_owner(&db, "m");
        let before = db
            .scope(module_id)
            .lookup(NameContext::Value, &ident("b"))
            .unique()
            .expect("b should resolve uniquely");

        // A net declaration does not change b's same-kind sibling occurrence,
        // so its stable source identity and definition must not move.
        db.set_file_text_with_durability(
            TOP,
            Arc::from("module m;\nwire w;\nint b;\nendmodule\n"),
            Durability::LOW,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let after = db
            .scope(module_id)
            .lookup(NameContext::Value, &ident("b"))
            .unique()
            .expect("b should still resolve uniquely");
        assert_eq!(before, after);
    }
    fn reference_at(db: &TestDb, text: &str, marker: &str, kind: RefKind) -> NameRef {
        let file_id = HirFileId::File(TOP);
        let tree = db.parse(file_id);
        let offset: u32 =
            u32::try_from(text.find(marker).expect("marker must exist")).expect("offset fits");
        let root = tree.root();
        let mut target = None;
        for event in root.node_preorder() {
            let syntax::WalkEvent::Enter(node) = event else { continue };
            let Some(range) = node.text_range() else { continue };
            if u32::from(range.start()) <= offset && offset <= u32::from(range.end()) {
                target = Some(node);
            }
        }
        let node = target.expect("node at marker");
        let ast_id = db.ast_id_map(file_id).id_of_node(node).expect("node must have an ast id");
        NameRef { position: InFile::new(file_id, ast_id), kind }
    }

    #[test]
    fn position_resolves_example_4_wildcard_order() {
        // IEEE 1800-2017 26.3 Example 4: only the wildcard import lexically
        // preceding the reference is considered.
        let text = r#"
package p;
function int f();
return 1;
endfunction
endpackage
package p2;
function int f();
return 1;
endfunction
endpackage
module top;
import p::*;
int x;
if (1) begin : b
  initial x = f();
end
import p2::*;
endmodule
"#;
        let db = db_with_root_text(text);
        let b = db
            .owner_table(HirFileId::File(TOP))
            .owners_of_kind(OwnerKind::GenerateBlock)
            .find(|owner| owner.name.as_str() == "b")
            .expect("generate block b")
            .id;
        let p = crate::unit::test_package_owner(&db, "p");
        let p_f = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            p,
            &ident("f"),
            NameContext::Value,
        )
        .unique()
        .expect("p::f");

        let reference = reference_at(&db, text, "x = f()", RefKind::Call);
        let resolved = resolve_name_at(
            &db,
            &crate::unit::test_resolution(&db),
            b,
            &ident("f"),
            NameContext::Value,
            Some(&reference),
        );
        assert_eq!(resolved, Resolution::Unique(p_f), "only the preceding wildcard may bind");

        // Without a position both wildcards merge (the previous behavior).
        let positionless = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            b,
            &ident("f"),
            NameContext::Value,
        );
        assert!(matches!(positionless, Resolution::Ambiguous(_)));
    }

    #[test]
    fn import_after_reference_is_ignored() {
        // IEEE 1800-2017 26.3 Example 3: an import after the reference point
        // does not bind the reference.
        let text = r#"
package p;
function int f();
return 1;
endfunction
endpackage
module top;
if (1) begin : b
  initial x = f();
  import p::*;
end
endmodule
"#;
        let db = db_with_root_text(text);
        let b = db
            .owner_table(HirFileId::File(TOP))
            .owners_of_kind(OwnerKind::GenerateBlock)
            .find(|owner| owner.name.as_str() == "b")
            .expect("generate block b")
            .id;

        let reference = reference_at(&db, text, "x = f()", RefKind::Call);
        assert!(
            resolve_name_at(
                &db,
                &crate::unit::test_resolution(&db),
                b,
                &ident("f"),
                NameContext::Value,
                Some(&reference)
            )
            .is_unresolved(),
            "the import follows the reference and must not bind"
        );
    }

    #[test]
    fn outer_scope_declarations_are_point_filtered() {
        // IEEE 1800-2017 26.3 Example 1: a reference activates the wildcard
        // import instead of a later declaration in an outer scope, so the
        // later declaration is not locally visible at the reference.
        let text = r#"
package p;
int x;
endpackage
module top;
import p::*;
if (1) begin : b
  initial x = 1;
end
int x;
endmodule
"#;
        let db = db_with_root_text(text);
        let b = db
            .owner_table(HirFileId::File(TOP))
            .owners_of_kind(OwnerKind::GenerateBlock)
            .find(|owner| owner.name.as_str() == "b")
            .expect("generate block b")
            .id;
        let p = crate::unit::test_package_owner(&db, "p");
        let p_x = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            p,
            &ident("x"),
            NameContext::Value,
        )
        .unique()
        .expect("p::x");

        let reference = reference_at(&db, text, "x = 1", RefKind::Value);
        assert_eq!(
            resolve_name_at(
                &db,
                &crate::unit::test_resolution(&db),
                b,
                &ident("x"),
                NameContext::Value,
                Some(&reference)
            ),
            Resolution::Unique(p_x),
            "the later outer declaration must not shadow the wildcard import"
        );
    }

    #[test]
    fn declaration_after_reference_is_not_visible() {
        let text = "module m;\ninitial begin : blk\n  x = 1;\n  int x;\nend\nendmodule\n";
        let db = db_with_root_text(text);
        let blk = db
            .owner_table(HirFileId::File(TOP))
            .owners_of_kind(OwnerKind::Block)
            .find(|owner| owner.name.as_str() == "blk")
            .expect("block blk")
            .id;

        let reference = reference_at(&db, text, "x = 1", RefKind::Value);
        assert!(
            resolve_name_at(
                &db,
                &crate::unit::test_resolution(&db),
                blk,
                &ident("x"),
                NameContext::Value,
                Some(&reference)
            )
            .is_unresolved(),
            "a declaration after the reference is not locally visible at the point"
        );
        assert!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                blk,
                &ident("x"),
                NameContext::Value
            )
            .unique()
            .is_some(),
            "position-less lookup keeps the declaration"
        );
    }

    #[test]
    fn call_reference_searches_to_scope_end() {
        // A function/task call sees declarations to the end of the innermost
        // scope (IEEE 1800-2017 26.3), unlike ordinary references.
        let text =
            "module m;\n  assign y = f();\n  function int f(); return 1; endfunction\nendmodule\n";
        let db = db_with_root_text(text);
        let m = crate::unit::test_module_owner(&db, "m");
        let f = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            m,
            &ident("f"),
            NameContext::Value,
        )
        .unique()
        .expect("m::f");

        let call = reference_at(&db, text, "y = f()", RefKind::Call);
        assert_eq!(
            resolve_name_at(
                &db,
                &crate::unit::test_resolution(&db),
                m,
                &ident("f"),
                NameContext::Value,
                Some(&call)
            ),
            Resolution::Unique(f)
        );
        let value = reference_at(&db, text, "y = f()", RefKind::Value);
        assert!(
            resolve_name_at(
                &db,
                &crate::unit::test_resolution(&db),
                m,
                &ident("f"),
                NameContext::Value,
                Some(&value)
            )
            .is_unresolved(),
            "ordinary references do not see the later declaration"
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
    fn hierarchy_root_fallback_ignores_nested_modules() {
        // Only compilation-unit module declarations may act as hierarchy
        // roots; a module nested inside a generate block must not leak.
        let db = db_with_root_text(
            r#"
module top;
  generate
    begin : g
      module child;
        wire sig;
      endmodule
    end
  endgenerate
endmodule
"#,
        );

        let resolution = resolve_path(
            &db,
            &crate::unit::test_resolution(&db),
            db.owner_table(HirFileId::File(TOP)).file_owner().expect("file owner"),
            &path(&["child", "sig"]),
            NameContext::Value,
        );
        assert_eq!(resolution, Resolution::Unresolved);
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

        let top = crate::unit::test_module_owner(&db, "top");

        let res = resolve_path(
            &db,
            &crate::unit::test_resolution(&db),
            top,
            &path(&["u_if", "host"]),
            NameContext::Value,
        );

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

        let top = crate::unit::test_module_owner(&db, "top");

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

        let top = crate::unit::test_module_owner(&db, "top");

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

        let top = crate::unit::test_module_owner(&db, "top");

        assert_eq!(resolved_kind(&db, top, &["u", "cp"], NameContext::Value), DefKind::Coverpoint);
        assert_eq!(resolved_kind(&db, top, &["u", "cx"], NameContext::Value), DefKind::Cross);
    }
}
