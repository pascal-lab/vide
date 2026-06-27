use smallvec::SmallVec;
use syntax::{SyntaxNode, SyntaxTokenWithParent};
use triomphe::Arc;
use utils::get::GetRef;

use super::SemanticsImpl;
use crate::{
    container::{InContainer, InFile, ScopeId, ScopeParent},
    db::HirDb,
    def_id::{ModuleDef, ModuleDefId},
    file::HirFileId,
    hir_def::{
        Ident, lower_ident_opt,
        module::{ModuleId, instantiation::InstanceId},
    },
    symbol::{DefId, DefKind, NameContext, NameScope},
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
    ) -> Option<PathResolution> {
        let ident = lower_ident_opt(Some(tok))?;
        self.with_ctx(|source_ctx| {
            let container = source_ctx.find_container(InFile::new(file_id, parent));
            source_ctx.name_to_def(InContainer::new(container, ident), name_ctx)
        })
    }

    pub(in crate::semantics) fn find_container(&self, node: InFile<SyntaxNode>) -> ScopeId {
        self.with_ctx(|ctx| ctx.find_container(node))
    }

    pub fn resolve_name(
        &self,
        cont_id: ScopeId,
        ident: &Ident,
        ctx: NameContext,
    ) -> Option<PathResolution> {
        resolve_name(self.db, cont_id, ident, ctx)
    }
}

pub fn resolve_name(
    db: &dyn HirDb,
    cont_id: ScopeId,
    ident: &Ident,
    ctx: NameContext,
) -> Option<PathResolution> {
    let scopes = ScopeParent::start_from(db, cont_id).collect::<SmallVec<[_; 4]>>();

    for id in &scopes {
        let scope = name_scope(db, *id);
        if let Some(res) = scope.lookup(ctx, ident).and_then(PathResolution::from_def_ids) {
            return Some(res);
        }
    }

    // IEEE 1800-2017 keeps package imports distinct from ordinary lexical
    // declarations: visible declarations in the lexical chain win, then
    // package imports are considered, and `$unit` remains an explicit outer
    // scope. `NameContext` chooses the namespace bucket at every phase.
    if let Some(res) = resolve_imported_name(db, &scopes, ident, ctx) {
        return Some(res);
    }

    db.unit_scope().lookup(ctx, ident).and_then(PathResolution::from_def_ids)
}

pub fn resolve_path(
    db: &dyn HirDb,
    cont_id: ScopeId,
    path: &[Ident],
    ctx: NameContext,
) -> Option<PathResolution> {
    let (first, rest) = path.split_first()?;
    let mut current = resolve_name(db, cont_id, first, ctx)
        .or_else(|| resolve_top_level_module_root(db, cont_id, first, ctx, !rest.is_empty()))?;

    for (idx, segment) in rest.iter().enumerate() {
        let segment_ctx = if idx + 1 == rest.len() { ctx } else { NameContext::Value };
        current = resolve_child_name(db, &current, segment, segment_ctx)?;
    }

    Some(current)
}

fn resolve_top_level_module_root(
    db: &dyn HirDb,
    cont_id: ScopeId,
    ident: &Ident,
    ctx: NameContext,
    has_child_segment: bool,
) -> Option<PathResolution> {
    if !has_child_segment || ctx != NameContext::Value {
        return None;
    }

    // IEEE 1800 hierarchical names can start at a top-level module instance.
    // Vide has module definitions in the type namespace and no separate
    // elaborated top-instance DefId yet, so a multi-segment value path may use
    // a module definition as an explicit hierarchy root. This is not a single
    // segment value fallback: `top` alone remains a type-space module name.
    let type_res = resolve_name(db, cont_id, ident, NameContext::Type)?;
    let module_defs =
        type_res.def_ids().iter().copied().filter(|def_id| def_id.kind(db) == DefKind::Module);
    PathResolution::from_def_ids(module_defs)
}

fn resolve_child_name(
    db: &dyn HirDb,
    parent: &PathResolution,
    ident: &Ident,
    ctx: NameContext,
) -> Option<PathResolution> {
    let mut defs = SmallVec::<[DefId; 3]>::new();
    for def_id in parent.def_ids() {
        let Some(scope_id) = descend_scope(db, *def_id) else {
            continue;
        };
        let Some(child_defs) = name_scope(db, scope_id).lookup(ctx, ident) else {
            continue;
        };
        for child_def_id in child_defs {
            if !defs.contains(&child_def_id) {
                defs.push(child_def_id);
            }
        }
    }
    PathResolution::from_def_ids(defs)
}

pub fn descend_scope(db: &dyn HirDb, def_id: DefId) -> Option<ScopeId> {
    match def_id.kind(db) {
        DefKind::Module => def_id.as_module(db).map(Into::into),
        DefKind::Instance => {
            let instance = def_id.as_instance(db)?;
            instance_target_module_id(db, instance.module_id, instance.value).map(Into::into)
        }
        DefKind::Block => def_id.as_block(db).map(Into::into),
        DefKind::GenerateBlock => def_id.as_generate_block(db).map(Into::into),
        _ => None,
    }
}

pub(crate) fn instance_target_module_id(
    db: &dyn HirDb,
    module_id: ModuleId,
    instance_id: InstanceId,
) -> Option<ModuleId> {
    let module = db.module(module_id);
    let instance = module.get(instance_id);
    let instantiation = module.get(instance.parent);
    let module_name = instantiation.module_name.as_ref()?;
    db.unit_scope().module_ids(db, module_name).unique()
}

pub(crate) fn name_scope(db: &dyn HirDb, scope_id: ScopeId) -> Arc<NameScope> {
    match scope_id {
        ScopeId::File(file_id) => db.file_scope(file_id),
        ScopeId::Module(module_id) => db.module_scope(module_id),
        ScopeId::GenerateBlock(generate_block_id) => db.generate_block_scope(generate_block_id),
        ScopeId::Block(block_id) => db.block_scope(block_id),
        ScopeId::Subroutine(subroutine_id) => db.subroutine_scope(subroutine_id.as_in_container()),
    }
}

fn resolve_imported_name(
    db: &dyn HirDb,
    scopes: &[ScopeId],
    ident: &Ident,
    ctx: NameContext,
) -> Option<PathResolution> {
    let mut defs = SmallVec::<[DefId; 3]>::new();

    for scope_id in scopes {
        let scope = name_scope(db, *scope_id);
        collect_imports(db, &scope, ident, ctx, true, &mut defs);
        if !defs.is_empty() {
            return PathResolution::from_def_ids(defs);
        }
    }

    for scope_id in scopes {
        let scope = name_scope(db, *scope_id);
        collect_imports(db, &scope, ident, ctx, false, &mut defs);
        if !defs.is_empty() {
            return PathResolution::from_def_ids(defs);
        }
    }

    None
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

        let Some(package_id) = db.unit_scope().package_ids(db, &import.package).unique() else {
            continue;
        };
        let package_scope = db.package_export_scope(package_id);
        if let Some(imported) = package_scope.lookup(ctx, ident) {
            for def_id in imported {
                if !defs.contains(&def_id) {
                    defs.push(def_id);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathResolution {
    def_ids: SmallVec<[DefId; 3]>,
}

impl PathResolution {
    pub fn from_def_id(def_id: DefId) -> Self {
        Self { def_ids: SmallVec::from_slice(&[def_id]) }
    }

    pub fn from_def_ids(def_ids: impl IntoIterator<Item = DefId>) -> Option<Self> {
        let mut resolved = SmallVec::<[DefId; 3]>::new();
        for def_id in def_ids {
            if !resolved.contains(&def_id) {
                resolved.push(def_id);
            }
        }
        (!resolved.is_empty()).then_some(Self { def_ids: resolved })
    }

    pub fn def_ids(&self) -> &[DefId] {
        &self.def_ids
    }

    pub fn primary_def_id(&self) -> Option<DefId> {
        self.def_ids.first().copied()
    }

    pub fn to_def_id(&self, db: &dyn HirDb) -> Option<ModuleDefId> {
        let module_def = ModuleDef::from_def_ids(self.def_ids.iter().copied())?;
        Some(db.intern_module_def(module_def))
    }
}

impl From<DefId> for PathResolution {
    fn from(def_id: DefId) -> Self {
        Self::from_def_id(def_id)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use rustc_hash::FxHashSet;
    use smol_str::SmolStr;
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{FileId, FileSet, VfsPath, anchored_path::AnchoredPath};

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

    const TOP: FileId = FileId(0);
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
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor_id);
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
        db.set_file_preprocess_config_with_durability(TOP, Arc::new(preprocess), Durability::LOW);
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
            .and_then(|res| res.primary_def_id())
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

        let res = resolve_path(&db, top.into(), &path(&["u_if", "host"]), NameContext::Value)
            .expect("interface instance modport should resolve");

        let def = res.primary_def_id().expect("modport should produce a definition");
        assert_eq!(def.name(&db).as_deref(), Some("host"));
        assert_eq!(
            resolved_kind(&db, top.into(), &["u_if", "clk"], NameContext::Value),
            DefKind::Net
        );
    }
}
