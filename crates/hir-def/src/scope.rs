use la_arena::Arena;
use preproc_expand::file::HirFileId;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::get::GetRef;

use crate::{
    PackageImport,
    body::Body,
    checker::CheckerPortId,
    container::{InFile, OwnerRef},
    db::HirDefDb,
    def_id::DefId,
    expr::declarator::DeclaratorParent,
    module::{
        clocking::ClockingSignalId,
        port::{PortDeclId, Ports},
    },
    owner::{OwnerId, OwnerKind},
    stmt::StmtKind,
    subroutine::SubroutinePortId,
    symbol::{DefOriginLoc, Import, NameContext, ScopeData},
};

// SystemVerilog has separate namespaces. This scope stores current supported
// declarations as:
// - types: modules, interfaces, packages, programs, typedefs
// - values: nets, variables, params, ports, subroutines, instances, blocks
// - assertions: reserved for sequence/property/checker work
// Hierarchical lookup remains a separate resolver path.

fn def_id(db: &dyn HirDefDb, loc: impl Into<DefOriginLoc>) -> DefId {
    DefId::from_source(db, loc)
}
fn owner_def_id(db: &dyn HirDefDb, owner: OwnerId) -> DefId {
    DefId::from_owner(db, owner).expect("owner must have a named definition origin")
}

/// Builds one lexical scope and invalidates only when that owner changes.
#[salsa::tracked(lru = 128, returns(clone))]
pub fn scope_for(db: &dyn HirDefDb, owner: OwnerId) -> Arc<ScopeData> {
    Arc::new(build_owner_scope(db, owner))
}

/// Builds the explicit `$unit` scope from each compilation-unit file scope.
///
/// Design-unit names come from the owner table. File-scope values, typedefs,
/// and imports still come from the file-owner body. Child module bodies are
/// not lowered here.
#[salsa::tracked(lru = 128, returns(clone))]
pub fn unit_scope(db: &dyn HirDefDb) -> Arc<ScopeData> {
    let mut unit = ScopeData::default();
    for file_id in compilation_unit_files(db) {
        let hir_file = HirFileId::File(file_id);
        if !db.file_facts(file_id).has_compilation_unit_locals() {
            continue;
        }
        let file_owner =
            db.owner_table(hir_file).file_owner().expect("owner table must contain file owner");
        unit.extend_definitions_from(scope_for(db, file_owner).as_ref());
    }
    Arc::new(unit)
}

fn compilation_unit_files(db: &dyn HirDefDb) -> Vec<vfs::FileId> {
    let mut files: Vec<_> = db
        .files()
        .iter()
        .copied()
        .filter(|&file_id| db.file_kind(file_id).is_semantic_compilation_unit())
        .collect();
    files.sort_by_key(|file_id| file_id.index());
    files
}

pub(crate) fn set_scope_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    scope_for::set_lru_capacity(db, capacity);
    unit_scope::set_lru_capacity(db, capacity);
}

fn body_scope(body: &Body, owner: OwnerId) -> &crate::body::BodyScopeData {
    body.scope(owner).expect("body must contain every requested lexical scope")
}

fn insert_body_declarators(
    scope: &mut ScopeData,
    db: &dyn HirDefDb,
    cont_id: OwnerId,
    body: &Body,
    owner: OwnerId,
) {
    for &decl_id in body_scope(body, owner).declarators() {
        let decl = &body.decls[decl_id];
        scope.insert_value_opt(&decl.name, def_id(db, OwnerRef::new(cont_id, decl_id)));
    }
}

fn insert_specify_declarators(
    scope: &mut ScopeData,
    db: &dyn HirDefDb,
    owner: OwnerId,
    body: &Body,
) {
    for specify_block in body.specify_blocks.values() {
        for item in &specify_block.items {
            let crate::module::specify::SpecifyBlockItem::DeclarationId(declaration_id) = item
            else {
                continue;
            };
            let declaration = &body.declarations[*declaration_id];
            for decl_id in declaration.decls() {
                let decl = &body.decls[decl_id];
                scope.insert_value_opt(&decl.name, def_id(db, OwnerRef::new(owner, decl_id)));
            }
        }
    }
}

fn insert_body_typedefs(
    scope: &mut ScopeData,
    db: &dyn HirDefDb,
    cont_id: OwnerId,
    body: &Body,
    owner: OwnerId,
) {
    for &typedef_id in body_scope(body, owner).typedefs() {
        let typedef = &body.typedefs[typedef_id];
        scope.insert_type_opt(&typedef.name, def_id(db, OwnerRef::new(cont_id, typedef_id)));
    }
}

fn insert_body_statements(
    scope: &mut ScopeData,
    db: &dyn HirDefDb,
    cont_id: OwnerId,
    body: &Body,
    owner: OwnerId,
) {
    for &stmt_id in body_scope(body, owner).statements() {
        let stmt = &body.stmts[stmt_id];
        scope.insert_value_opt(&stmt.label, def_id(db, OwnerRef::new(cont_id, stmt_id)));
        if let StmtKind::Block(block_owner) = stmt.kind {
            scope.insert_value_opt(
                &block_owner.name(db),
                def_id(db, DefOriginLoc::Block(block_owner)),
            );
        }
    }
}

fn insert_proc_bodies(scope: &mut ScopeData, db: &dyn HirDefDb, procs: &Arena<crate::proc::Proc>) {
    for (_, proc) in procs.iter() {
        let body = db.body_with_source_map(proc.owner);
        insert_body_statements(scope, db, proc.owner, body.data_ref(), proc.owner);
    }
}

impl ScopeData {
    pub fn non_ansi_port_decl_id_by_name(
        &self,
        db: &dyn HirDefDb,
        body: &crate::body::Body,
        name: &SmolStr,
    ) -> Option<PortDeclId> {
        let def = self.lookup(NameContext::Value, name).unique()?;
        def.origins(db).into_iter().filter_map(|origin| origin.as_decl(db)).find_map(|decl_id| {
            let decl = &body.decls[decl_id.value];
            match decl.parent {
                DeclaratorParent::PortDeclId(port_decl_id) => Some(port_decl_id),
                _ => None,
            }
        })
    }

    fn insert_package_import(&mut self, import: &PackageImport) {
        self.insert_import(Import {
            package: import.package.clone(),
            name: import.item.clone(),
            source: import.source,
        });
    }
}

pub(crate) fn build_file_scope(db: &dyn HirDefDb, file_id: HirFileId) -> ScopeData {
    let mut scope = ScopeData::default();
    let owner_table = db.owner_table(file_id);
    let file_owner = owner_table.file_owner().expect("file owner must exist");

    // Compilation-unit design units and file-scope subroutines are on the
    // owner table. Projecting them through `from_owner` must not lower a
    // child body: that is what made `$unit` pay for every module in the
    // project.
    for owner in owner_table.owners() {
        if owner.parent != Some(file_owner) {
            continue;
        }
        let name = (!owner.name.is_empty()).then(|| owner.name.clone());
        match owner.kind {
            OwnerKind::Module => {
                // Compilation-unit design units live on UnitCatalog, not in
                // the file / $unit lexical scope.
            }
            OwnerKind::Subroutine => {
                if let Some(def) = DefId::from_owner(db, owner.id) {
                    scope.insert_value_opt(&name, def);
                }
            }
            _ => {}
        }
    }

    let hir_file = db.body(file_owner);
    let body = db.body_with_source_map(file_owner);

    for (_, import) in hir_file.package_imports.iter() {
        scope.insert_package_import(import);
    }

    insert_body_declarators(&mut scope, db, file_owner, body.data_ref(), file_owner);
    insert_body_typedefs(&mut scope, db, file_owner, body.data_ref(), file_owner);
    insert_proc_bodies(&mut scope, db, &hir_file.procs);

    for (config_decl_id, config_decl) in hir_file.config_decls.iter() {
        scope.insert_value_opt(&config_decl.name, def_id(db, InFile::new(file_id, config_decl_id)));
    }
    for (udp_decl_id, udp_decl) in hir_file.udp_decls.iter() {
        scope.insert_value_opt(&udp_decl.name, def_id(db, InFile::new(file_id, udp_decl_id)));
    }
    for (library_decl_id, library_decl) in hir_file.library_decls.iter() {
        scope.insert_value_opt(
            &library_decl.name,
            def_id(db, InFile::new(file_id, library_decl_id)),
        );
    }
    for item in &hir_file.items {
        match item {
            crate::body::BodyItem::CheckerOwner(owner) => {
                let checker = owner
                    .as_checker(db)
                    .expect("checker owner must contain a lowered checker definition");
                let body = db.body(*owner);
                let checker_data = body.get(checker.value);
                scope.insert_type_opt(&checker_data.name, def_id(db, checker));
            }
            crate::body::BodyItem::CovergroupOwner(owner) => {
                let covergroup = owner
                    .as_covergroup(db)
                    .expect("covergroup owner must contain a lowered covergroup definition");
                let body = db.body(*owner);
                let covergroup_data = body.get(covergroup.value);
                scope.insert_type_opt(&covergroup_data.name, def_id(db, covergroup));
            }
            _ => {}
        }
    }
    scope
}

pub(crate) fn build_module_scope(db: &dyn HirDefDb, owner: OwnerId) -> ScopeData {
    let mut scope = ScopeData::default();
    let module = db.body(owner);
    let body = db.body_with_source_map(owner);

    if let Ports::NonAnsi { ports, .. } = &module.ports {
        for (port_id, port) in ports.iter() {
            scope.insert_value_opt(&port.label, def_id(db, OwnerRef::new(owner, port_id)));
        }
    }
    for (_, import) in module.package_imports.iter() {
        scope.insert_package_import(import);
    }
    for subroutine_owner in module.subroutine_owners() {
        let subroutine = db.subroutine(subroutine_owner);
        scope.insert_value_opt(&subroutine.name, owner_def_id(db, subroutine_owner));
    }
    for (modport_id, modport) in module.modports.iter() {
        scope.insert_value_opt(&modport.name, def_id(db, OwnerRef::new(owner, modport_id)));
    }
    for item in &module.items {
        match item {
            crate::body::BodyItem::ClockingBlockOwner(owner) => {
                let clocking = owner
                    .as_clocking_block(db)
                    .expect("clocking owner must contain a lowered clocking definition");
                let body = db.body(*owner);
                let clocking_data = body.get(clocking.value);
                scope.insert_value_opt(&clocking_data.name, def_id(db, clocking));
            }
            crate::body::BodyItem::CheckerOwner(owner) => {
                let checker = owner
                    .as_checker(db)
                    .expect("checker owner must contain a lowered checker definition");
                let body = db.body(*owner);
                let checker_data = body.get(checker.value);
                scope.insert_type_opt(&checker_data.name, def_id(db, checker));
            }
            crate::body::BodyItem::CovergroupOwner(owner) => {
                let covergroup = owner
                    .as_covergroup(db)
                    .expect("covergroup owner must contain a lowered covergroup definition");
                let body = db.body(*owner);
                let covergroup_data = body.get(covergroup.value);
                scope.insert_type_opt(&covergroup_data.name, def_id(db, covergroup));
            }
            crate::body::BodyItem::PropertyId(property_id) => {
                let property = module.get(*property_id);
                if let Some(name) = &property.name {
                    scope.insert_assertion(name, def_id(db, OwnerRef::new(owner, *property_id)));
                }
            }
            crate::body::BodyItem::SequenceId(sequence_id) => {
                let sequence = module.get(*sequence_id);
                if let Some(name) = &sequence.name {
                    scope.insert_assertion(name, def_id(db, OwnerRef::new(owner, *sequence_id)));
                }
            }
            _ => {}
        }
    }
    insert_body_declarators(&mut scope, db, owner, body.data_ref(), owner);
    insert_specify_declarators(&mut scope, db, owner, body.data_ref());
    insert_body_typedefs(&mut scope, db, owner, body.data_ref(), owner);
    for (instance_id, instance) in module.instances.iter() {
        scope.insert_value_opt(&instance.name, def_id(db, OwnerRef::new(owner, instance_id)));
    }
    for item in &module.items {
        if let crate::body::BodyItem::GenerateRegionId(generate_region_id) = item {
            let generate_region = module.get(*generate_region_id);
            for item in &generate_region.items {
                if let crate::body::BodyItem::GenerateBlockOwner(generate_block_owner) = item {
                    let generate_block = db.body(*generate_block_owner);
                    scope.insert_value_opt(
                        &generate_block.name,
                        owner_def_id(db, *generate_block_owner),
                    );
                }
            }
        }
    }
    insert_body_statements(&mut scope, db, owner, body.data_ref(), owner);
    insert_proc_bodies(&mut scope, db, &module.procs);
    scope
}

pub(crate) fn build_clocking_block_scope(db: &dyn HirDefDb, owner: OwnerId) -> ScopeData {
    let mut scope = ScopeData::default();
    let clocking_id = owner
        .as_clocking_block(db)
        .expect("clocking owner must contain a lowered clocking definition");
    let body = db.body(owner);
    let clocking_block = body.get(clocking_id.value);
    for (idx, signal) in clocking_block.signals.iter().enumerate() {
        let signal_id = ClockingSignalId(idx as u32);
        scope.insert_value(&signal.name, def_id(db, OwnerRef::new(owner, signal_id)));
    }
    scope
}

pub(crate) fn build_checker_scope(db: &dyn HirDefDb, owner: OwnerId) -> ScopeData {
    let mut scope = ScopeData::default();
    let checker_id =
        owner.as_checker(db).expect("checker owner must contain a lowered checker definition");
    let checker = db.body(owner).get(checker_id.value).clone();
    for (idx, port) in checker.ports.iter().enumerate() {
        let port_id = CheckerPortId(idx as u32);
        scope.insert_value(&port.name, def_id(db, OwnerRef::new(owner, port_id)));
    }
    let lowered = db.body_with_source_map(owner);
    for declaration_id in &checker.declarations {
        let declaration = &lowered.data_ref().declarations[*declaration_id];
        for decl_id in declaration.decls() {
            let decl = &lowered.data_ref().decls[decl_id];
            scope.insert_value_opt(&decl.name, def_id(db, OwnerRef::new(owner, decl_id)));
        }
    }
    scope
}

pub(crate) fn build_covergroup_scope(db: &dyn HirDefDb, owner: OwnerId) -> ScopeData {
    let mut scope = ScopeData::default();
    let covergroup_id = owner
        .as_covergroup(db)
        .expect("covergroup owner must contain a lowered covergroup definition");
    let body = db.body(owner);
    let covergroup = body.get(covergroup_id.value);
    for coverpoint_id in &covergroup.coverpoints {
        let coverpoint = body.get(*coverpoint_id);
        scope.insert_value_opt(&coverpoint.name, def_id(db, OwnerRef::new(owner, *coverpoint_id)));
    }
    for cross_id in &covergroup.crosses {
        let cross = body.get(*cross_id);
        scope.insert_value_opt(&cross.name, def_id(db, OwnerRef::new(owner, *cross_id)));
    }
    scope
}

pub(crate) fn build_generate_block_scope(db: &dyn HirDefDb, owner: OwnerId) -> ScopeData {
    let mut scope = ScopeData::default();
    let body = db.body_with_source_map(owner);
    scope.insert_value_opt(&body.name, owner_def_id(db, owner));
    for (_, import) in body.package_imports.iter() {
        scope.insert_package_import(import);
    }
    for subroutine_owner in body.subroutine_owners() {
        let subroutine = db.subroutine(subroutine_owner);
        scope.insert_value_opt(&subroutine.name, owner_def_id(db, subroutine_owner));
    }
    insert_body_declarators(&mut scope, db, owner, body.data_ref(), owner);
    insert_body_typedefs(&mut scope, db, owner, body.data_ref(), owner);
    for (instance_id, instance) in body.instances.iter() {
        scope.insert_value_opt(&instance.name, def_id(db, OwnerRef::new(owner, instance_id)));
    }
    for item in &body.items {
        if let crate::body::BodyItem::GenerateBlockOwner(child_owner) = item {
            let child = db.body(*child_owner);
            scope.insert_value_opt(&child.name, owner_def_id(db, *child_owner));
        }
    }
    insert_body_statements(&mut scope, db, owner, body.data_ref(), owner);
    insert_proc_bodies(&mut scope, db, &body.procs);
    scope
}

pub(crate) fn build_subroutine_scope(db: &dyn HirDefDb, owner: OwnerId) -> ScopeData {
    let mut scope = ScopeData::default();
    let subroutine = db.subroutine(owner);
    let body = db.body_with_source_map(owner);

    for (port_idx, port) in subroutine.ports.iter().enumerate() {
        let port_id = SubroutinePortId(port_idx as u32);
        scope.insert_value_opt(&port.name, def_id(db, OwnerRef::new(owner, port_id)));
    }

    insert_body_declarators(&mut scope, db, owner, body.data_ref(), owner);
    insert_body_typedefs(&mut scope, db, owner, body.data_ref(), owner);
    insert_body_statements(&mut scope, db, owner, body.data_ref(), owner);
    scope
}

pub(crate) fn build_block_scope(db: &dyn HirDefDb, owner: OwnerId) -> ScopeData {
    let body = db.body_with_source_map(owner);
    let mut scope = ScopeData::default();
    insert_body_declarators(&mut scope, db, owner, body.data_ref(), owner);
    insert_body_typedefs(&mut scope, db, owner, body.data_ref(), owner);
    insert_body_statements(&mut scope, db, owner, body.data_ref(), owner);
    scope
}

pub(crate) fn build_owner_scope(db: &dyn HirDefDb, owner: OwnerId) -> ScopeData {
    match owner.kind(db) {
        OwnerKind::File => build_file_scope(db, owner.file(db)),
        OwnerKind::Module => build_module_scope(db, owner),
        OwnerKind::AnonymousProgram => build_module_scope(db, owner),
        OwnerKind::GenerateBlock => build_generate_block_scope(db, owner),
        OwnerKind::Block => build_block_scope(db, owner),
        OwnerKind::Subroutine => build_subroutine_scope(db, owner),
        OwnerKind::ProceduralBlock => {
            let body = db.body_with_source_map(owner);
            let mut scope = ScopeData::default();
            insert_body_declarators(&mut scope, db, owner, body.data_ref(), owner);
            insert_body_typedefs(&mut scope, db, owner, body.data_ref(), owner);
            insert_body_statements(&mut scope, db, owner, body.data_ref(), owner);
            scope
        }
        OwnerKind::Checker => build_checker_scope(db, owner),
        OwnerKind::Covergroup => build_covergroup_scope(db, owner),
        OwnerKind::ClockingBlock => build_clocking_block_scope(db, owner),
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
    use syntax::ast::{self, AstNode};
    use triomphe::Arc;
    use utils::{
        get::{Get, GetRef},
        paths::{AbsPathBuf, Utf8PathBuf},
    };
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use crate::{
        Ident,
        db::HirDefDb,
        def_id::DefId,
        has_source::HasSource,
        module::port::{PortSrcs, Ports},
        pathres::resolve_name,
        symbol::{DefKind, DefOriginLoc, NameContext, Resolution, ScopeKind},
        unit::ToOwner,
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

    #[test]
    fn scope_kind_reflects_module_declaration_kind() {
        let db = db_with_root_text(
            r#"
package p;
endpackage
interface i;
endinterface
program pr;
endprogram
module m;
endmodule
"#,
        );
        let file_id = HirFileId::File(TOP);
        let file = db.body(db.owner_table(file_id).file_owner().expect("file owner"));
        let actual = file
            .module_owners()
            .map(|owner| {
                let module = db.body(owner);
                (module.name.clone().unwrap(), owner.scope_kind(&db))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                (ident("p"), ScopeKind::Package),
                (ident("i"), ScopeKind::Interface),
                (ident("pr"), ScopeKind::Program),
                (ident("m"), ScopeKind::Module),
            ]
        );
    }

    #[test]
    fn scope_graph_context_lookup_covers_current_scope_shapes() {
        let db = db_with_root_text(
            r#"
typedef logic shared;
wire shared;
wire file_sig;

module m(a);
  output a;
  reg [7:0] a;

  function automatic [3:0] f(input p);
    begin: b
      integer x;
    end
  endfunction

  generate
    if (1) begin: g
      wire y;
    end
  endgenerate
endmodule
"#,
        );

        let unit_scope = db.unit_scope();
        let module_in_unit = DefId::from_owner(&db, crate::unit::test_module_owner(&db, "m"))
            .expect("compilation-unit module projects");
        assert_eq!(module_in_unit.kind(&db), DefKind::Module);
        assert!(
            unit_scope.lookup(NameContext::Type, &ident("m")).is_unresolved(),
            "design-unit names are not $unit locals"
        );
        assert!(
            unit_scope
                .lookup(NameContext::Value, &ident("file_sig"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Net)
        );
        let shared_defs = unit_scope.lookup(NameContext::Listing, &ident("shared"));
        assert!(shared_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Typedef));
        assert!(shared_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Net));
        let shared_type_defs = unit_scope.lookup(NameContext::Type, &ident("shared"));
        assert!(shared_type_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Typedef));
        assert!(!shared_type_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Net));
        let shared_value_defs = unit_scope.lookup(NameContext::Value, &ident("shared"));
        assert!(shared_value_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Net));
        assert!(!shared_value_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Typedef));

        let module_id = crate::unit::test_module_owner(&db, "m");
        assert_eq!(module_id.file(&db), HirFileId::File(TOP));

        let module_scope = db.scope(module_id);
        let port_def = module_scope
            .lookup(NameContext::Value, &ident("a"))
            .unique()
            .expect("non-ANSI port name should resolve uniquely");
        assert_eq!(port_def.kind(&db), DefKind::Port);
        assert!(
            port_def
                .source(&db)
                .expect("definition should retain its source")
                .value
                .focus_range()
                .is_some()
        );
        assert!(
            port_def.origins(&db).iter().any(|origin| origin.kind(&db) == DefKind::NonAnsiPort)
        );
        assert!(port_def.origins(&db).iter().any(|origin| origin.kind(&db) == DefKind::Port));
        assert!(port_def.origins(&db).iter().any(|origin| origin.kind(&db) == DefKind::Variable));

        let subroutine_id = module_scope
            .lookup(NameContext::Value, &ident("f"))
            .iter()
            .find_map(|def_id| def_id.primary_origin(&db).as_subroutine(&db))
            .expect("subroutine should be visible from module scope");
        assert!(
            subroutine_id.source(&db).is_some_and(|source| source.value.focus_range().is_some())
        );
        let subroutine_scope = db.scope(subroutine_id);
        assert!(
            subroutine_scope
                .lookup(NameContext::Value, &ident("p"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::SubroutinePort)
        );

        let block_id = subroutine_scope
            .lookup(NameContext::Value, &ident("b"))
            .iter()
            .find_map(|def_id| def_id.primary_origin(&db).as_block(&db))
            .expect("named block should be visible from subroutine scope");
        assert!(
            block_id
                .source(&db)
                .expect("block should retain its source")
                .value
                .focus_range()
                .is_some()
        );
        assert!(
            db.scope(block_id)
                .lookup(NameContext::Value, &ident("x"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Variable)
        );

        let generate_block_id = module_scope
            .lookup(NameContext::Value, &ident("g"))
            .iter()
            .find_map(|def_id| def_id.primary_origin(&db).as_generate_block(&db))
            .expect("generate block should be visible from module scope");
        assert!(
            generate_block_id
                .source(&db)
                .is_some_and(|source| source.value.focus_range().is_some())
        );
        assert!(
            db.scope(generate_block_id)
                .lookup(NameContext::Value, &ident("y"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Net)
        );

        // Adding an interface lowering should create a DefKind::Interface
        // producer and insert the resulting DefId into ScopeGraph; IDE
        // feature matches already have default no-op arms.
    }

    #[test]
    fn lookup_distinguishes_ambiguous_definitions_from_multiple_origins() {
        let db = db_with_root_text(
            r#"
wire duplicate;
wire duplicate;

module m(a);
  output a;
  reg [7:0] a;
endmodule
"#,
        );

        let duplicate = db.unit_scope().lookup(NameContext::Value, &ident("duplicate"));
        let crate::symbol::Resolution::Ambiguous(candidates) = duplicate else {
            panic!("same-name declarations should remain ambiguous");
        };
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|def| def.origins(&db).len() == 1));

        let module_id = crate::unit::test_module_owner(&db, "m");
        let port = db
            .scope(module_id)
            .lookup(NameContext::Value, &ident("a"))
            .unique()
            .expect("one logical non-ANSI port should resolve uniquely");
        let origins = port.origins(&db);
        assert_eq!(origins.len(), 3);
        assert_eq!(port.declaration_origin(&db).kind(&db), DefKind::Port);
        for origin in origins {
            assert_eq!(DefId::from_source(&db, origin.loc(&db).clone()), port);
        }
    }

    #[test]
    fn unit_scope_module_def_id_matches_body_backed_projection() {
        let db = db_with_root_text(
            r#"
module m;
  logic buried;
endmodule
"#,
        );
        let owner = crate::unit::test_module_owner(&db, "m");
        let from_header = DefId::from_owner(&db, owner).expect("module owner has a definition");
        assert_eq!(from_header, DefId::from_source(&db, DefOriginLoc::Module(owner)));
        assert_eq!(from_header.name(&db).as_deref(), Some("m"));
        assert!(db.unit_scope().lookup(NameContext::Value, &ident("buried")).is_unresolved());
    }

    #[test]
    fn explicit_non_ansi_port_source_preserves_name_range() {
        let db = db_with_root_text(
            r#"
module m(.out(foo));
  output foo;
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let Ports::NonAnsi { ports, .. } = &module.ports else {
            panic!("module should have non-ANSI ports");
        };
        let (port_id, _) = ports.iter().next().expect("port should lower");

        assert!(
            module.source_name_range(&db, port_id).is_some(),
            "explicit port name range should be preserved"
        );
    }

    #[test]
    fn implicit_non_ansi_port_source_supports_natural_reverse_lookup() {
        let db = db_with_root_text(
            r#"
module m(foo);
  output foo;
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let source_map = module.source_map();
        let Ports::NonAnsi { ports, .. } = &module.ports else {
            panic!("module should have non-ANSI ports");
        };
        let (port_id, _) = ports.iter().next().expect("port should lower");

        let tree = db.parse(TOP.into());
        let root = tree.root();
        let unit = ast::CompilationUnit::cast(root).expect("root should be a compilation unit");
        let ast::Member::ModuleDeclaration(module_ast) =
            unit.members().children().next().expect("module should parse")
        else {
            panic!("first member should be a module");
        };
        let ast::PortList::NonAnsiPortList(port_list) =
            module_ast.header().ports().expect("module should have a port list")
        else {
            panic!("module should have a non-ANSI port list");
        };
        let port_ast = port_list.ports().children().next().expect("port should parse");
        let natural_source = db
            .ast_id_map(TOP.into())
            .id_of_node_in_tree(&tree, port_ast.syntax())
            .expect("port AST node should have a source identity");
        let PortSrcs::NonAnsi { ports: port_sources, .. } = &source_map.port_srcs else {
            panic!("source map should contain non-ANSI ports");
        };

        assert_eq!(
            port_sources.src_to_hir(natural_source),
            Some(port_id),
            "natural AST source key should resolve to the port"
        );
    }

    #[test]
    fn non_ansi_port_def_id_is_stable_when_origins_change() {
        let mut db = db_with_root_text(
            r#"
module m(a);
  output a;
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let before = db
            .scope(module_id)
            .lookup(NameContext::Value, &ident("a"))
            .unique()
            .expect("port should resolve uniquely");
        let declaration_origin = before
            .origins(&db)
            .into_iter()
            .find(|origin| matches!(origin.loc(&db), DefOriginLoc::Decl(_)))
            .expect("non-ANSI port must retain its declaration origin");
        assert_eq!(
            before,
            DefId::from_source(&db, declaration_origin.loc(&db).clone()),
            "declaration and port source identities must resolve to one local definition"
        );
        assert_eq!(before.origins(&db).len(), 2);

        db.set_file_text_with_durability(
            TOP,
            Arc::from(
                r#"
module m(a);
  output a;
  reg [7:0] a;
endmodule
"#,
            ),
            Durability::LOW,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let after = db
            .scope(module_id)
            .lookup(NameContext::Value, &ident("a"))
            .unique()
            .expect("port should still resolve uniquely");
        assert_eq!(after.origins(&db).len(), 3);
        assert_eq!(before, after);
    }

    #[test]
    fn non_ansi_port_does_not_absorb_unrelated_parameter() {
        let db = db_with_root_text(
            r#"
module m(a);
  input a;
  parameter a = 1;
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let Resolution::Ambiguous(candidates) =
            db.scope(module_id).lookup(NameContext::Value, &ident("a"))
        else {
            panic!("the port and parameter should remain separate definitions");
        };
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|def| def.kind(&db) == DefKind::Port));
        assert!(candidates.iter().any(|def| def.kind(&db) == DefKind::Param));

        let port = candidates.iter().find(|def| def.kind(&db) == DefKind::Port).unwrap();
        assert!(port.origins(&db).iter().all(|origin| origin.kind(&db) != DefKind::Param));
    }

    #[test]
    fn duplicate_non_ansi_labels_do_not_claim_the_same_declaration() {
        let db = db_with_root_text(
            r#"
module m(a, a);
  input a;
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let Resolution::Ambiguous(candidates) =
            db.scope(module_id).lookup(NameContext::Value, &ident("a"))
        else {
            panic!("duplicate labels should remain ambiguous");
        };
        assert_eq!(candidates.len(), 3);
        assert!(candidates.iter().all(|def| def.origins(&db).len() == 1));
    }

    #[test]
    fn duplicate_non_ansi_data_declarations_remain_ambiguous() {
        let db = db_with_root_text(
            r#"
module m(a);
  input a;
  reg a;
  reg a;
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let Resolution::Ambiguous(candidates) =
            db.scope(module_id).lookup(NameContext::Value, &ident("a"))
        else {
            panic!("duplicate data declarations should remain ambiguous");
        };
        assert_eq!(candidates.len(), 3);
        let port = candidates.iter().find(|def| def.kind(&db) == DefKind::Port).unwrap();
        assert_eq!(port.origins(&db).len(), 2);
        assert_eq!(candidates.iter().filter(|def| def.kind(&db) == DefKind::Variable).count(), 2);
    }

    #[test]
    fn assignment_pattern_lowering_retains_source_without_diagnostic() {
        let db = db_with_root_text(
            r#"
module m;
  int x = '{default: 0};
endmodule
"#,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let owner = module_id;
        let module = db.body_with_source_map(owner);
        let (expr_id, expr) = module
            .exprs
            .iter()
            .find(|(_, expr)| matches!(expr, crate::expr::Expr::AssignmentPattern { .. }))
            .expect("assignment pattern expression should lower");

        assert!(matches!(expr, crate::expr::Expr::AssignmentPattern { .. }));
        assert!(module.source(expr_id).is_some(), "lowered expressions must retain their source");
        assert!(
            module.diagnostics(&db).is_empty(),
            "supported assignment patterns must not emit diagnostics"
        );
    }

    #[test]
    fn common_rtl_statements_lower_to_structured_hir() {
        let db = db_with_root_text(
            r#"
module m;
  logic [3:0] arr [0:3];
  logic x;
  initial begin
    foreach (arr[i]) x = x inside {[0:3]};
    wait fork;
    wait_order (x) x = 1;
  end
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let proc = module.procs.iter().next().expect("initial block should lower").1;
        let body = db.body_with_source_map(proc.owner);

        assert!(
            body.stmts
                .values()
                .any(|stmt| matches!(stmt.kind, crate::stmt::StmtKind::Foreach { .. }))
        );
        assert!(
            body.stmts.values().any(|stmt| matches!(stmt.kind, crate::stmt::StmtKind::WaitFork))
        );
        assert!(
            body.stmts
                .values()
                .any(|stmt| matches!(stmt.kind, crate::stmt::StmtKind::WaitOrder { .. }))
        );
        assert!(body.exprs.values().any(|expr| matches!(expr, crate::expr::Expr::Inside { .. })));
        assert!(body.diagnostics(&db).is_empty(), "supported RTL statements must not diagnose");
    }

    #[test]
    fn v11_expression_foreach_and_hierarchical_cross_lower() {
        let db = db_with_root_text(
            r#"
module m(input logic clk, input logic a);
  logic [3:0] da2 [0:3];
  initial begin
    foreach (m::self().da2[i]) da2[i] = a;
  end
  covergroup cg @(posedge clk);
    cp: coverpoint a;
    cx: cross m::cp, cp;
  endgroup
endmodule
"#,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let proc = module.procs.iter().next().expect("initial block should lower").1;
        let body = db.body_with_source_map(proc.owner);
        assert!(
            body.stmts
                .values()
                .any(|stmt| matches!(stmt.kind, crate::stmt::StmtKind::Foreach { .. }))
        );
        assert!(body.diagnostics(&db).is_empty(), "v11 foreach expression must lower cleanly");

        let covergroup_owner = module
            .items
            .iter()
            .find_map(|item| match item {
                crate::body::BodyItem::CovergroupOwner(owner) => Some(*owner),
                _ => None,
            })
            .expect("covergroup owner should lower");
        let covergroup_body = db.body_with_source_map(covergroup_owner);
        let covergroup = covergroup_body
            .data_ref()
            .covergroups
            .values()
            .next()
            .expect("covergroup definition should lower");
        assert_eq!(covergroup.crosses.len(), 1);
        let cross = covergroup_body.get(covergroup.crosses[0]);
        assert_eq!(cross.items.len(), 2);
    }

    #[test]
    fn assertion_statements_lower_with_actions() {
        let db = db_with_root_text(
            r#"
module m;
  logic clk, x, y;
  always @(posedge clk) begin
    assert (x) x = 1; else x = 0;
    assert property (x |-> y) x = 1; else x = 0;
  end
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let proc = module.procs.iter().next().expect("always block should lower").1;
        let body = db.body_with_source_map(proc.owner);
        let assertions: Vec<_> = body
            .stmts
            .values()
            .filter(|stmt| {
                matches!(
                    stmt.kind,
                    crate::stmt::StmtKind::ImmediateAssertion { .. }
                        | crate::stmt::StmtKind::ConcurrentAssertion { .. }
                )
            })
            .collect();
        assert_eq!(assertions.len(), 2);
        assert!(assertions.iter().all(|stmt| match stmt.kind {
            crate::stmt::StmtKind::ImmediateAssertion { action, .. }
            | crate::stmt::StmtKind::ConcurrentAssertion { action, .. } =>
                action.pass.is_some() && action.fail.is_some(),
            _ => false,
        }));
        assert!(body.diagnostics(&db).is_empty(), "supported assertions must not diagnose");
    }

    #[test]
    fn module_assertion_member_lowers_to_body_item() {
        let db = db_with_root_text(
            r#"
module m;
  logic x, y;
  assert property (x |-> y);
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        assert!(
            module
                .items
                .iter()
                .any(|item| matches!(item, crate::body::BodyItem::AssertionStmtId(_)))
        );
        assert!(
            module
                .stmts
                .values()
                .any(|stmt| matches!(stmt.kind, crate::stmt::StmtKind::ConcurrentAssertion { .. }))
        );
        assert!(
            module.diagnostics(&db).is_empty(),
            "supported assertion members must not diagnose"
        );
    }

    #[test]
    fn module_scope_exposes_property_and_sequence_declarations() {
        let db = db_with_root_text(
            r#"
module m(input logic x, y);
  property p;
    x |-> y;
  endproperty
  sequence s;
    x ##1 y;
  endsequence
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        assert!(
            module.items.iter().any(|item| matches!(item, crate::body::BodyItem::PropertyId(_)))
        );
        assert!(
            module.items.iter().any(|item| matches!(item, crate::body::BodyItem::SequenceId(_)))
        );

        let scope = db.scope(module_id);
        let property = scope
            .lookup(NameContext::Assertion, &ident("p"))
            .unique()
            .expect("property should resolve in assertion namespace");
        assert_eq!(property.kind(&db), DefKind::Property);
        assert!(property.primary_origin(&db).source(&db).is_some());

        let sequence = scope
            .lookup(NameContext::Assertion, &ident("s"))
            .unique()
            .expect("sequence should resolve in assertion namespace");
        assert_eq!(sequence.kind(&db), DefKind::Sequence);
        assert!(sequence.primary_origin(&db).source(&db).is_some());
        assert!(module.diagnostics(&db).is_empty(), "supported declarations must not diagnose");
    }

    #[test]
    fn struct_data_type_lowers_without_diagnostic() {
        let db = db_with_root_text(
            r#"
module m;
  struct { logic x; } value;
endmodule
"#,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let declaration = module
            .declarations
            .values()
            .find(|declaration| matches!(declaration, crate::declaration::Declaration::DataDecl(_)))
            .expect("struct variable declaration should lower");
        assert!(matches!(declaration.ty(), crate::expr::data_ty::DataTy::Struct(_)));
        assert!(
            module.diagnostics(&db).is_empty(),
            "supported struct types must not emit diagnostics"
        );
    }

    #[test]
    fn parser_missing_and_empty_statements_are_distinct() {
        let db = db_with_root_text(
            r#"
module m;
  initial ;
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let proc = module.procs.iter().next().expect("initial block should lower").1;
        let body = db.body_with_source_map(proc.owner);
        let (empty_id, _) = body
            .stmts
            .iter()
            .find(|(_, stmt)| matches!(stmt.kind, crate::stmt::StmtKind::Empty))
            .expect("empty statement should lower");
        assert!(body.source(empty_id).is_some());

        let mut missing_body = crate::body::Body::default();
        let mut missing_body_source_map = crate::body::BodySourceMap::default();
        let file_owner =
            db.owner_table(HirFileId::File(TOP)).file_owner().expect("file owner must exist");
        let mut ctx = crate::lower::LoweringCtx::new(
            &db,
            file_owner,
            crate::lower::BodyStore {
                data: &mut missing_body,
                sources: &mut missing_body_source_map,
            },
        );
        let missing_id = ctx.lower_stmt_opt(None);
        drop(ctx);

        assert!(matches!(missing_body.stmts[missing_id].kind, crate::stmt::StmtKind::Missing));
        assert!(missing_body_source_map.stmt_srcs.get(missing_id).is_none());
    }

    #[test]
    fn streaming_with_range_is_preserved() {
        let db = db_with_root_text(
            r#"
module m(input logic [3:0] a);
  logic [3:0] x = {<<{a with [3:0]}};
endmodule
"#,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let owner = module_id;
        let module = db.body_with_source_map(owner);
        let stream = module
            .exprs
            .iter()
            .find_map(|(_, expr)| match expr {
                crate::expr::Expr::Stream { concats, .. } => Some(concats),
                _ => None,
            })
            .expect("streaming concatenation should lower");

        assert_eq!(stream.len(), 1);
        assert!(matches!(
            stream[0].with_range.as_ref().and_then(|range| range.selector),
            Some(crate::expr::Selector::Range(_, _))
        ));
    }

    #[test]
    fn invalid_streaming_with_range_is_preserved() {
        let db = db_with_root_text(
            r#"
module m(input logic [3:0] a);
  logic [3:0] x = {<<{a, a with [], a with [3:0]}};
endmodule
"#,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let owner = module_id;
        let module = db.body_with_source_map(owner);
        let stream = module
            .exprs
            .iter()
            .find_map(|(_, expr)| match expr {
                crate::expr::Expr::Stream { concats, .. } => Some(concats),
                _ => None,
            })
            .expect("streaming concatenation should lower");

        assert_eq!(stream.len(), 3);
        assert!(stream[0].with_range.is_none(), "an omitted with range must remain absent");
        assert!(
            stream[1].with_range.as_ref().is_some_and(|range| range.selector.is_none()),
            "the present but invalid with range must retain a missing selector"
        );
        assert!(matches!(
            stream[2].with_range.as_ref().and_then(|range| range.selector),
            Some(crate::expr::Selector::Range(_, _))
        ));
    }

    #[test]
    fn module_scope_exposes_clocking_blocks() {
        let db = db_with_root_text(
            r#"
module m(input clk, input a);
  clocking cb @(posedge clk);
    input #1ps a;
  endclocking
  default clocking cb;
endmodule
"#,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let clocking_owner = module
            .items
            .iter()
            .find_map(|item| match item {
                crate::body::BodyItem::ClockingBlockOwner(owner) => Some(*owner),
                _ => None,
            })
            .expect("clocking block owner should lower");
        let clocking_block_id = clocking_owner
            .as_clocking_block(&db)
            .expect("clocking block owner should contain a block")
            .value;
        let clocking_body = db.body_with_source_map(clocking_owner);
        let clocking_block = clocking_body.get(clocking_block_id);
        assert_eq!(clocking_block.name.as_deref(), Some("cb"));
        assert!(matches!(
            clocking_body.data_ref().event_exprs[clocking_block.event],
            crate::expr::timing_control::EventExpr::Atom {
                sensitivity: Some(crate::expr::timing_control::Sensitivity::Posedge),
                ..
            }
        ));
        assert!(clocking_body.source(clocking_block.event).is_some());
        assert_eq!(
            module.default_clocking.as_ref().and_then(|reference| reference.name.as_deref()),
            Some("cb")
        );
        assert!(module.source_map().default_clocking_src.is_some());
        assert_eq!(clocking_block.signals.len(), 1);
        assert_eq!(clocking_block.signals[0].name.as_str(), "a");

        let defs = db.scope(module_id).lookup(NameContext::Value, &ident("cb"));
        assert!(defs.iter().any(|def_id| {
            def_id.kind(&db) == DefKind::ClockingBlock
                && def_id
                    .primary_origin(&db)
                    .as_clocking_block(&db)
                    .is_some_and(|id| id.value == clocking_block_id)
        }));
    }

    #[test]
    fn file_scope_exposes_checkers_and_lowers_checker_instances() {
        let db = db_with_root_text(
            r#"
checker c(input logic clk);
  logic sig;
endchecker

module m;
  c u();
endmodule
"#,
        );

        let checker_owner = crate::unit::test_graph(&db)
            .type_units_named("c")
            .unique()
            .expect("checker is a graph node")
            .to_owner(&db)
            .expect("checker projects");
        let checker_defs = DefId::from_owner(&db, checker_owner).map(Resolution::Unique).unwrap();
        assert!(checker_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Checker));
        let checker_id = checker_defs
            .iter()
            .cloned()
            .find_map(|def_id| def_id.primary_origin(&db).as_checker(&db))
            .expect("checker definition should have a concrete id");
        let owner = DefOriginLoc::Checker(checker_id).owner(&db);
        let checker_scope = db.scope(owner);
        assert!(
            checker_scope
                .lookup(NameContext::Value, &ident("clk"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::CheckerPort)
        );
        assert!(
            checker_scope
                .lookup(NameContext::Value, &ident("sig"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Variable)
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body(module_id);
        let instantiation = module
            .instantiations
            .values()
            .find(|instantiation| instantiation.module_name.as_deref() == Some("c"))
            .expect("checker instantiation should lower into the instance arena");
        let instance = instantiation
            .instances
            .first()
            .map(|instance_id| module.get(*instance_id))
            .expect("checker instantiation should lower its instance");
        assert_eq!(instance.name.as_deref(), Some("u"));
    }

    #[test]
    fn module_scope_exposes_covergroups_and_coverage_items() {
        let db = db_with_root_text(
            r#"
module m(input clk, input a);
  covergroup cg @(posedge clk);
    cp: coverpoint a;
    cx: cross cp, cp;
  endgroup

  cg u();
endmodule
"#,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body(module_id);
        let covergroup_owner = module
            .items
            .iter()
            .find_map(|item| match item {
                crate::body::BodyItem::CovergroupOwner(owner) => Some(*owner),
                _ => None,
            })
            .expect("covergroup owner should lower");
        let covergroup_id = covergroup_owner
            .as_covergroup(&db)
            .expect("covergroup owner should contain a covergroup")
            .value;
        let covergroup_body = db.body_with_source_map(covergroup_owner);
        let covergroup = covergroup_body.get(covergroup_id);
        assert_eq!(covergroup.name.as_deref(), Some("cg"));
        assert_eq!(covergroup.coverpoints.len(), 1);
        assert_eq!(covergroup.crosses.len(), 1);

        let coverpoint_id = covergroup.coverpoints[0];
        let cross_id = covergroup.crosses[0];
        assert_eq!(covergroup_body.get(coverpoint_id).name.as_deref(), Some("cp"));
        assert_eq!(covergroup_body.get(cross_id).name.as_deref(), Some("cx"));

        let module_scope = db.scope(module_id);
        let covergroup_defs = module_scope.lookup(NameContext::Type, &ident("cg"));
        assert!(covergroup_defs.iter().any(|def_id| {
            def_id.kind(&db) == DefKind::Covergroup
                && def_id
                    .primary_origin(&db)
                    .as_covergroup(&db)
                    .is_some_and(|id| id.cont_id == covergroup_owner && id.value == covergroup_id)
        }));

        let covergroup_scope = db.scope(covergroup_owner);
        let scoped_coverpoint_defs = covergroup_scope.lookup(NameContext::Value, &ident("cp"));
        assert!(scoped_coverpoint_defs.iter().any(|def_id| {
            matches!(
                def_id.primary_origin(&db).loc(&db),
                DefOriginLoc::Coverpoint(id)
                    if id.cont_id == covergroup_owner && id.value == coverpoint_id
            )
        }));
        let scoped_cross_defs = covergroup_scope.lookup(NameContext::Value, &ident("cx"));
        assert!(scoped_cross_defs.iter().any(|def_id| {
            matches!(
                def_id.primary_origin(&db).loc(&db),
                DefOriginLoc::Cross(id)
                    if id.cont_id == covergroup_owner && id.value == cross_id
            )
        }));

        let instantiation = module
            .instantiations
            .values()
            .find(|instantiation| instantiation.module_name.as_deref() == Some("cg"))
            .expect("covergroup instantiation should lower into the instance arena");
        let instance = instantiation
            .instances
            .first()
            .map(|instance_id| module.get(*instance_id))
            .expect("covergroup instantiation should lower its instance");
        assert_eq!(instance.name.as_deref(), Some("u"));
    }

    #[test]
    fn package_imports_resolve_through_export_scope() {
        let db = db_with_root_text(
            r#"
package pkg;
  typedef logic imported_t;
  int imported_v;
  int shadowed_v;
  function int imported_f();
    return 1;
  endfunction
endpackage

module wildcard_importer;
  import pkg::*;
  wire shadowed_v;
endmodule

module named_importer;
  import pkg::imported_v;
endmodule
"#,
        );

        let package_id = crate::unit::test_package_owner(&db, "pkg");
        let package_exports = db.package_exports(&crate::unit::test_resolution(&db), package_id);
        assert!(
            package_exports
                .lookup(NameContext::Type, &ident("imported_t"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Typedef)
        );
        assert!(
            package_exports
                .lookup(NameContext::Value, &ident("imported_v"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Variable)
        );
        assert!(
            package_exports
                .lookup(NameContext::Value, &ident("imported_f"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Subroutine)
        );

        let wildcard_importer = crate::unit::test_module_owner(&db, "wildcard_importer");
        let wildcard_scope = db.scope(wildcard_importer);
        assert!(
            wildcard_scope
                .imports()
                .iter()
                .any(|import| import.package == ident("pkg") && import.name.is_none())
        );

        let imported_t = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            wildcard_importer,
            &ident("imported_t"),
            NameContext::Type,
        );
        assert!(imported_t.iter().any(|def_id| def_id.kind(&db) == DefKind::Typedef));
        assert!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                wildcard_importer,
                &ident("imported_t"),
                NameContext::Value,
            )
            .is_unresolved(),
            "value lookup should not fall back to the type bucket"
        );

        let shadowed_v = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            wildcard_importer,
            &ident("shadowed_v"),
            NameContext::Value,
        );
        assert!(shadowed_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Net));
        assert!(!shadowed_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Variable));

        let named_importer = crate::unit::test_module_owner(&db, "named_importer");
        let named_scope = db.scope(named_importer);
        assert!(named_scope.imports().iter().any(|import| {
            import.package == ident("pkg")
                && import.name.as_ref().is_some_and(|name| name == "imported_v")
        }));

        let imported_v = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            named_importer,
            &ident("imported_v"),
            NameContext::Value,
        );
        assert!(imported_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Variable));
        assert!(
            resolve_name(
                &db,
                &crate::unit::test_resolution(&db),
                named_importer,
                &ident("imported_t"),
                NameContext::Type,
            )
            .is_unresolved(),
            "named import should not expose unrelated package symbols"
        );
    }

    #[test]
    fn package_subroutine_def_id_is_canonical_across_imports() {
        let db = db_with_root_text(
            r#"
package pkg;
  function automatic int f();
    return 1;
  endfunction
endpackage

module named_importer;
  import pkg::f;
endmodule

module wildcard_importer;
  import pkg::*;
endmodule
"#,
        );

        let package_id = crate::unit::test_package_owner(&db, "pkg");
        let package_f = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            package_id,
            &ident("f"),
            NameContext::Value,
        )
        .unique()
        .expect("package scope should resolve package subroutine");

        let DefOriginLoc::Subroutine(package_subroutine) = package_f.primary_origin(&db).loc(&db)
        else {
            panic!("package f should resolve to a subroutine");
        };
        assert_eq!(package_subroutine.parent(&db), Some(package_id));

        let named_importer = crate::unit::test_module_owner(&db, "named_importer");
        let named_import_f = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            named_importer,
            &ident("f"),
            NameContext::Value,
        )
        .unique()
        .expect("named import should resolve package subroutine");

        let wildcard_importer = crate::unit::test_module_owner(&db, "wildcard_importer");
        let wildcard_import_f = resolve_name(
            &db,
            &crate::unit::test_resolution(&db),
            wildcard_importer,
            &ident("f"),
            NameContext::Value,
        )
        .unique()
        .expect("wildcard import should resolve package subroutine");

        assert_eq!(package_f, named_import_f);
        assert_eq!(
            package_f.primary_origin(&db).loc(&db),
            named_import_f.primary_origin(&db).loc(&db)
        );
        assert_eq!(package_f, wildcard_import_f);
        assert_eq!(
            package_f.primary_origin(&db).loc(&db),
            wildcard_import_f.primary_origin(&db).loc(&db)
        );
    }

    #[test]
    fn declaration_def_id_survives_preceding_declaration_insertion() {
        let mut db = db_with_root_text(
            r#"
module m;
  logic stable;
endmodule
"#,
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let before = db
            .scope(module_id)
            .lookup(NameContext::Value, &ident("stable"))
            .unique()
            .expect("stable declaration should resolve uniquely");

        db.set_file_text_with_durability(
            TOP,
            Arc::from(
                r#"
module m;
  logic inserted;
  logic stable;
endmodule
"#,
            ),
            Durability::LOW,
        );

        let module_id = crate::unit::test_module_owner(&db, "m");
        let after = db
            .scope(module_id)
            .lookup(NameContext::Value, &ident("stable"))
            .unique()
            .expect("stable declaration should still resolve uniquely");
        assert_eq!(before, after, "a preceding declaration must not shift DefId identity");
    }

    #[test]
    fn owner_scoped_lookup_survives_another_owner_edit() {
        let mut db = db_with_root_text(
            r#"
module first;
  logic first_value;
endmodule

module second;
  logic second_value;
endmodule
"#,
        );
        let second = crate::unit::test_module_owner(&db, "second");
        let before = db
            .scope(second)
            .lookup(NameContext::Value, &ident("second_value"))
            .unique()
            .expect("second owner declaration should resolve");

        db.set_file_text_with_durability(
            TOP,
            Arc::from(
                r#"
module first;
  logic inserted;
  logic first_value;
endmodule

module second;
  logic second_value;
endmodule
"#,
            ),
            Durability::LOW,
        );

        let second = crate::unit::test_module_owner(&db, "second");
        let after = db
            .scope(second)
            .lookup(NameContext::Value, &ident("second_value"))
            .unique()
            .expect("second owner declaration should remain visible");
        assert_eq!(before, after);
    }

    #[test]
    fn package_export_signature_is_stable_across_function_body_edits() {
        let mut db = db_with_root_text(
            r#"
package pkg;
  typedef logic exported_t;
  int exported_v;
  function int exported_f();
    int body_local;
    return body_local;
  endfunction
endpackage
"#,
        );

        let package_id = crate::unit::test_package_owner(&db, "pkg");

        let exports = db.package_exports(&crate::unit::test_resolution(&db), package_id);
        assert!(
            exports
                .lookup(NameContext::Value, &ident("exported_f"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Subroutine)
        );

        let before_body_edit =
            db.package_export_signature(&crate::unit::test_resolution(&db), package_id);
        let before_design_map = crate::unit::test_resolution(&db).design_map(&db);
        db.set_file_text_with_durability(
            TOP,
            Arc::from(
                r#"
package pkg;
  typedef logic exported_t;
  int exported_v;
  function int exported_f();
    int changed_body_local;
    return changed_body_local;
  endfunction
endpackage
"#,
            ),
            Durability::LOW,
        );
        let after_body_edit =
            db.package_export_signature(&crate::unit::test_resolution(&db), package_id);
        assert_eq!(
            before_body_edit, after_body_edit,
            "function body edits should not change the package export signature"
        );
        let after_design_map = crate::unit::test_resolution(&db).design_map(&db);
        assert_eq!(
            before_design_map, after_design_map,
            "function body edits should not change the design map"
        );
    }

    #[test]
    fn param_port_index_aligns_with_type_parameter_positions() {
        let db = db_with_root_text(
            "module m #(parameter int A = 0, parameter type T = logic, parameter int B = 1) ();\nendmodule\n",
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let body = db.body(module_id);
        assert_eq!(crate::module::param_port_count(&body), 3);
        assert!(crate::module::param_port_id_by_idx(&body, 0).is_some(), "A");
        assert_eq!(crate::module::param_port_id_by_idx(&body, 1), None, "T has no declarator");
        assert!(crate::module::param_port_id_by_idx(&body, 2).is_some(), "B");
        assert_eq!(crate::module::param_port_id_by_idx(&body, 3), None, "out of range");
        // Ordinals stay aligned with the parameter list, not the declarators.
        let a = crate::module::param_port_id_by_idx(&body, 0).expect("A");
        let b = crate::module::param_port_id_by_idx(&body, 2).expect("B");
        assert_ne!(a, b);
    }

    #[test]
    fn default_nettype_selects_implicit_port_net_kind() {
        let db = db_with_root_text("`default_nettype tri\nmodule m(input a);\nendmodule\n");
        let module_id = crate::unit::test_module_owner(&db, "m");
        let body = db.body(module_id);
        let Ports::Ansi(port_decls) = &body.ports else {
            panic!("module should have ANSI ports");
        };
        let header = &port_decls.values().next().expect("port").header;
        assert!(
            matches!(
                header,
                crate::module::port::PortHeader::Net {
                    net_ty: crate::ty::NetType { kind: crate::ty::NetKind::Tri, .. },
                    ..
                }
            ),
            "implicit port net must honor `default_nettype tri`: {header:?}"
        );
    }

    #[test]
    fn default_nettype_directive_switches_mid_file() {
        let db = db_with_root_text(
            "`default_nettype tri\nmodule a(input x);\nendmodule\n`default_nettype wire\nmodule b(input y);\nendmodule\n",
        );
        let kinds = ["a", "b"].map(|name| {
            let module_id = crate::unit::test_module_owner(&db, name);
            let body = db.body(module_id);
            let Ports::Ansi(port_decls) = &body.ports else {
                panic!("module should have ANSI ports");
            };
            match &port_decls.values().next().expect("port").header {
                crate::module::port::PortHeader::Net { net_ty, .. } => net_ty.kind,
                _ => panic!("implicit net expected"),
            }
        });
        assert_eq!(
            kinds,
            [crate::ty::NetKind::Tri, crate::ty::NetKind::Wire],
            "each module honors the directive preceding it"
        );
    }

    #[test]
    fn interface_port_header_is_not_previous_header() {
        let db = db_with_root_text("module m(input logic a, interface.ifc);\nendmodule\n");
        let module_id = crate::unit::test_module_owner(&db, "m");
        let body = db.body(module_id);
        let Ports::Ansi(port_decls) = &body.ports else {
            panic!("module should have ANSI ports");
        };
        assert!(
            port_decls.values().any(|decl| decl.header
                == crate::module::port::PortHeader::Interface {
                    dir: crate::module::port::PortDirection::default(),
                }),
            "interface port must keep its own header instead of the previous one"
        );
    }

    #[test]
    fn scoped_select_keeps_scope_receiver() {
        let db = db_with_root_text(
            "package pkg;\nendpackage\nmodule m;\ninitial begin\nx = pkg::arr[0];\nend\nendmodule\n",
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let module = db.body_with_source_map(module_id);
        let (_, proc) = module.procs.iter().next().expect("initial block");
        let body = db.body_with_source_map(proc.owner);
        let found = body.exprs.values().any(|expr| {
            matches!(
                expr,
                crate::expr::Expr::ElementSelect { receiver, .. }
                    if matches!(
                        &body.exprs[*receiver],
                        crate::expr::Expr::Field { field, .. } if field.as_deref() == Some("arr")
                    )
            )
        });
        assert!(found, "pkg::arr[0] must keep its scope receiver");
    }

    #[test]
    fn overridable_param_index_skips_body_params() {
        let db = db_with_root_text(
            "module m #(parameter int A = 0, parameter int B = 1) ();\n  parameter int P = 2;\nendmodule\n",
        );
        let module_id = crate::unit::test_module_owner(&db, "m");
        let body = db.body(module_id);
        let a = crate::module::param_port_id_by_idx(&body, 0).expect("A");
        let b = crate::module::param_port_id_by_idx(&body, 1).expect("B");
        assert_eq!(crate::module::overridable_param_id_by_idx(&body, 0), Some(a));
        assert_eq!(crate::module::overridable_param_id_by_idx(&body, 1), Some(b));
        assert_eq!(crate::module::overridable_param_id_by_idx(&body, 2), None);
    }
}
