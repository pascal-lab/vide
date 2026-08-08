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
    symbol::{DefOriginLoc, Import, NameContext, NameScope},
};

// SystemVerilog has separate namespaces. This scope stores current supported
// declarations as:
// - types: modules, interfaces, packages, programs, typedefs
// - values: nets, variables, params, ports, subroutines, instances, blocks
// - assertions: reserved for sequence/property/checker work
// Hierarchical lookup remains a separate resolver path.

fn def_id(db: &dyn HirDefDb, loc: impl Into<DefOriginLoc>) -> DefId {
    DefId::new(db, loc)
}
fn owner_def_id(db: &dyn HirDefDb, owner: OwnerId) -> DefId {
    let origin = match owner.kind(db) {
        OwnerKind::Module => DefOriginLoc::Module(owner),
        OwnerKind::GenerateBlock => DefOriginLoc::GenerateBlock(owner),
        OwnerKind::Block => DefOriginLoc::Block(owner),
        OwnerKind::Subroutine => DefOriginLoc::Subroutine(owner),
        kind => panic!("owner {owner:?} of kind {kind:?} has no named definition origin"),
    };
    def_id(db, origin)
}

fn body_scope(body: &Body, owner: OwnerId) -> &crate::body::BodyScopeData {
    body.scope(owner).expect("body must contain every requested lexical scope")
}

fn insert_body_declarators(
    scope: &mut NameScope,
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
    scope: &mut NameScope,
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
    scope: &mut NameScope,
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
    scope: &mut NameScope,
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

fn insert_proc_bodies(scope: &mut NameScope, db: &dyn HirDefDb, procs: &Arena<crate::proc::Proc>) {
    for (_, proc) in procs.iter() {
        let body = db.body_with_source_map(proc.owner);
        insert_body_statements(scope, db, proc.owner, body.data_ref(), proc.owner);
    }
}

#[salsa::tracked]
impl NameScope {
    #[salsa::tracked(returns(clone))]
    pub fn unit_scope(db: &dyn HirDefDb) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        for file_id in db.files().iter() {
            let file_id = HirFileId::File(*file_id);
            let file_owner = db.owner_table(file_id).file_owner().expect("file owner");
            let file_scope = db.scope_for(file_owner);
            scope.extend_definitions_from(&file_scope);
        }
        Arc::new(scope)
    }

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
        self.insert_import(Import { package: import.package.clone(), name: import.item.clone() });
    }
}

pub(crate) fn build_file_scope(db: &dyn HirDefDb, file_id: HirFileId) -> NameScope {
    let mut scope = NameScope::default();
    let file_owner = db.owner_table(file_id).file_owner().expect("file owner must exist");
    let hir_file = db.body(file_owner);
    let body = db.body_with_source_map(file_owner);

    for owner in hir_file.module_owners() {
        let module = db.body(owner);
        scope.insert_type_opt(&module.name, def_id(db, DefOriginLoc::Module(owner)));
    }

    for (_, import) in hir_file.package_imports.iter() {
        scope.insert_package_import(import);
    }

    insert_body_declarators(&mut scope, db, file_owner, body.data_ref(), file_owner);
    insert_body_typedefs(&mut scope, db, file_owner, body.data_ref(), file_owner);
    insert_proc_bodies(&mut scope, db, &hir_file.procs);

    for subroutine_owner in hir_file.subroutine_owners() {
        let subroutine = db.subroutine(subroutine_owner);
        scope.insert_value_opt(&subroutine.name, owner_def_id(db, subroutine_owner));
    }

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

pub(crate) fn build_module_scope(db: &dyn HirDefDb, owner: OwnerId) -> NameScope {
    let mut scope = NameScope::default();
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

pub(crate) fn build_clocking_block_scope(db: &dyn HirDefDb, owner: OwnerId) -> NameScope {
    let mut scope = NameScope::default();
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

pub(crate) fn build_checker_scope(db: &dyn HirDefDb, owner: OwnerId) -> NameScope {
    let mut scope = NameScope::default();
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

pub(crate) fn build_covergroup_scope(db: &dyn HirDefDb, owner: OwnerId) -> NameScope {
    let mut scope = NameScope::default();
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

pub(crate) fn build_generate_block_scope(db: &dyn HirDefDb, owner: OwnerId) -> NameScope {
    let mut scope = NameScope::default();
    let body = db.body_with_source_map(owner);
    scope.insert_value_opt(&body.name, owner_def_id(db, owner));
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

pub(crate) fn build_subroutine_scope(db: &dyn HirDefDb, owner: OwnerId) -> NameScope {
    let mut scope = NameScope::default();
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

pub(crate) fn build_block_scope(db: &dyn HirDefDb, owner: OwnerId) -> NameScope {
    let body = db.body_with_source_map(owner);
    let mut scope = NameScope::default();
    insert_body_declarators(&mut scope, db, owner, body.data_ref(), owner);
    insert_body_typedefs(&mut scope, db, owner, body.data_ref(), owner);
    insert_body_statements(&mut scope, db, owner, body.data_ref(), owner);
    scope
}

pub(crate) fn build_owner_scope(db: &dyn HirDefDb, owner: OwnerId) -> NameScope {
    match owner.kind(db) {
        OwnerKind::File => build_file_scope(db, owner.file(db)),
        OwnerKind::Module => build_module_scope(db, owner),
        OwnerKind::GenerateBlock => build_generate_block_scope(db, owner),
        OwnerKind::Block => build_block_scope(db, owner),
        OwnerKind::Subroutine => build_subroutine_scope(db, owner),
        OwnerKind::ProceduralBlock => {
            let body = db.body_with_source_map(owner);
            let mut scope = NameScope::default();
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
    fn name_scope_context_lookup_covers_current_scope_shapes() {
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

        let module_id = unit_scope
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        assert_eq!(module_id.file(&db), HirFileId::File(TOP));

        let module_scope = db.scope_for(module_id);
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
        let subroutine_scope = db.scope_for(subroutine_id);
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
            db.scope_for(block_id)
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
            db.scope_for(generate_block_id)
                .lookup(NameContext::Value, &ident("y"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Net)
        );

        // Adding an interface lowering should create a DefKind::Interface
        // producer and insert the resulting DefId into NameScope; IDE
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let port = db
            .scope_for(module_id)
            .lookup(NameContext::Value, &ident("a"))
            .unique()
            .expect("one logical non-ANSI port should resolve uniquely");
        let origins = port.origins(&db);
        assert_eq!(origins.len(), 3);
        assert_eq!(port.declaration_origin(&db).kind(&db), DefKind::Port);
        for origin in origins {
            assert_eq!(DefId::new(&db, origin.loc(&db).clone()), port);
        }
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
        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
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
        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let module = db.body_with_source_map(module_id);
        let source_map = module.source_map();
        let Ports::NonAnsi { ports, .. } = &module.ports else {
            panic!("module should have non-ANSI ports");
        };
        let (port_id, _) = ports.iter().next().expect("port should lower");

        let tree = db.parse(TOP.into());
        let root = tree.root().expect("source should parse");
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
        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let before = db
            .scope_for(module_id)
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
            DefId::new(&db, declaration_origin.loc(&db).clone()),
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should still resolve uniquely");
        let after = db
            .scope_for(module_id)
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
        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let Resolution::Ambiguous(candidates) =
            db.scope_for(module_id).lookup(NameContext::Value, &ident("a"))
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
        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let Resolution::Ambiguous(candidates) =
            db.scope_for(module_id).lookup(NameContext::Value, &ident("a"))
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
        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let Resolution::Ambiguous(candidates) =
            db.scope_for(module_id).lookup(NameContext::Value, &ident("a"))
        else {
            panic!("duplicate data declarations should remain ambiguous");
        };
        assert_eq!(candidates.len(), 3);
        let port = candidates.iter().find(|def| def.kind(&db) == DefKind::Port).unwrap();
        assert_eq!(port.origins(&db).len(), 2);
        assert_eq!(candidates.iter().filter(|def| def.kind(&db) == DefKind::Variable).count(), 2);
    }

    #[test]
    fn valid_unlowered_expression_is_not_parser_missing() {
        let db = db_with_root_text(
            r#"
module m;
  int x = '{default: 0};
endmodule
"#,
        );

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let owner = module_id;
        let module = db.body_with_source_map(owner);
        let (expr_id, expr) =
            module.exprs.iter().next().expect("initializer expression should lower");

        assert_eq!(
            expr,
            &crate::expr::Expr::Unsupported(syntax::SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION),
            "valid but unsupported syntax must carry an explicit diagnostic kind"
        );
        assert!(
            module.source(expr_id).is_some(),
            "valid but unsupported syntax must retain its source"
        );
        assert_eq!(module.diagnostics(&db).len(), 1);
        let diagnostic = &module.diagnostics(&db)[0];
        assert_eq!(diagnostic.kind, crate::source_map::LoweringDiagnosticKind::UnsupportedSyntax);
        assert_eq!(diagnostic.syntax_kind, syntax::SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION);
        assert!(diagnostic.range.is_some());
    }
    #[test]
    fn unsupported_module_member_is_reported_with_source() {
        let db = db_with_root_text(
            r#"
module m;
  property p;
  endproperty
endmodule
"#,
        );
        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let diagnostics = db.body_with_source_map(module_id).diagnostics(&db);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message == "assertion member is not lowered")
            .unwrap_or_else(|| {
                panic!("unsupported module member should be diagnosed: {diagnostics:?}")
            });
        assert_eq!(diagnostic.kind, crate::source_map::LoweringDiagnosticKind::UnsupportedSyntax);
        assert!(diagnostic.source.is_some(), "diagnostic must retain a source identity");
    }

    #[test]
    fn unsupported_data_type_is_recoverable_and_diagnosed() {
        let db = db_with_root_text(
            r#"
module m;
  struct { logic x; } value;
endmodule
"#,
        );

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let owner = module_id;
        let module = db.body_with_source_map(owner);
        let declaration = module
            .declarations
            .values()
            .find(|declaration| matches!(declaration, crate::declaration::Declaration::DataDecl(_)))
            .expect("struct variable declaration should lower");
        assert!(matches!(
            declaration.ty(),
            crate::expr::data_ty::DataTy::Unsupported(syntax::SyntaxKind::STRUCT_TYPE)
        ));
        assert!(
            module.diagnostics(&db).iter().any(|diagnostic| {
                diagnostic.kind == crate::source_map::LoweringDiagnosticKind::UnsupportedSyntax
                    && diagnostic.syntax_kind == syntax::SyntaxKind::STRUCT_TYPE
                    && diagnostic.range.is_some()
            }),
            "unsupported data types must retain provenance in lowering diagnostics"
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
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

        let defs = db.scope_for(module_id).lookup(NameContext::Value, &ident("cb"));
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

        let checker_defs = db.unit_scope().lookup(NameContext::Type, &ident("c"));
        assert!(checker_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Checker));
        let checker_id = checker_defs
            .iter()
            .cloned()
            .find_map(|def_id| def_id.primary_origin(&db).as_checker(&db))
            .expect("checker definition should have a concrete id");
        let owner = DefOriginLoc::Checker(checker_id).owner(&db);
        let checker_scope = db.scope_for(owner);
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
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

        let module_scope = db.scope_for(module_id);
        let covergroup_defs = module_scope.lookup(NameContext::Type, &ident("cg"));
        assert!(covergroup_defs.iter().any(|def_id| {
            def_id.kind(&db) == DefKind::Covergroup
                && def_id
                    .primary_origin(&db)
                    .as_covergroup(&db)
                    .is_some_and(|id| id.cont_id == covergroup_owner && id.value == covergroup_id)
        }));

        let covergroup_scope = db.scope_for(covergroup_owner);
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

        let unit_scope = db.unit_scope();
        let package_id = unit_scope
            .package_ids(&db, &ident("pkg"))
            .unique()
            .expect("package should resolve uniquely");
        let package_exports = db.package_exports(package_id);
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

        let wildcard_importer = db
            .unit_scope()
            .module_ids(&db, &ident("wildcard_importer"))
            .unique()
            .expect("wildcard importer should resolve uniquely");
        let wildcard_scope = db.scope_for(wildcard_importer);
        assert!(
            wildcard_scope
                .imports()
                .iter()
                .any(|import| import.package == ident("pkg") && import.name.is_none())
        );

        let imported_t =
            resolve_name(&db, wildcard_importer, &ident("imported_t"), NameContext::Type);
        assert!(imported_t.iter().any(|def_id| def_id.kind(&db) == DefKind::Typedef));
        assert!(
            resolve_name(&db, wildcard_importer, &ident("imported_t"), NameContext::Value,)
                .is_unresolved(),
            "value lookup should not fall back to the type bucket"
        );

        let shadowed_v =
            resolve_name(&db, wildcard_importer, &ident("shadowed_v"), NameContext::Value);
        assert!(shadowed_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Net));
        assert!(!shadowed_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Variable));

        let named_importer = db
            .unit_scope()
            .module_ids(&db, &ident("named_importer"))
            .unique()
            .expect("named importer should resolve uniquely");
        let named_scope = db.scope_for(named_importer);
        assert!(named_scope.imports().iter().any(|import| {
            import.package == ident("pkg")
                && import.name.as_ref().is_some_and(|name| name == "imported_v")
        }));

        let imported_v =
            resolve_name(&db, named_importer, &ident("imported_v"), NameContext::Value);
        assert!(imported_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Variable));
        assert!(
            resolve_name(&db, named_importer, &ident("imported_t"), NameContext::Type,)
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

        let package_id = db
            .unit_scope()
            .package_ids(&db, &ident("pkg"))
            .unique()
            .expect("package should resolve uniquely");
        let package_f = resolve_name(&db, package_id, &ident("f"), NameContext::Value)
            .unique()
            .expect("package scope should resolve package subroutine");

        let DefOriginLoc::Subroutine(package_subroutine) = package_f.primary_origin(&db).loc(&db)
        else {
            panic!("package f should resolve to a subroutine");
        };
        assert_eq!(package_subroutine.parent(&db), Some(package_id));

        let named_importer = db
            .unit_scope()
            .module_ids(&db, &ident("named_importer"))
            .unique()
            .expect("named importer should resolve uniquely");
        let named_import_f = resolve_name(&db, named_importer, &ident("f"), NameContext::Value)
            .unique()
            .expect("named import should resolve package subroutine");

        let wildcard_importer = db
            .unit_scope()
            .module_ids(&db, &ident("wildcard_importer"))
            .unique()
            .expect("wildcard importer should resolve uniquely");
        let wildcard_import_f =
            resolve_name(&db, wildcard_importer, &ident("f"), NameContext::Value)
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

        let package_id = db
            .unit_scope()
            .package_ids(&db, &ident("pkg"))
            .unique()
            .expect("package should resolve uniquely");

        let exports = db.package_exports(package_id);
        assert!(
            exports
                .lookup(NameContext::Value, &ident("exported_f"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Subroutine)
        );

        let before_body_edit = db.package_export_signature(package_id);
        let before_design_map = db.design_map();
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
        let after_body_edit = db.package_export_signature(package_id);
        assert_eq!(
            before_body_edit, after_body_edit,
            "function body edits should not change the package export signature"
        );
        let after_design_map = db.design_map();
        assert_eq!(
            before_design_map, after_design_map,
            "function body edits should not change the design map"
        );
    }
}
