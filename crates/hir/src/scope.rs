use smol_str::SmolStr;
use triomphe::Arc;
use utils::get::{Get, GetRef};

use crate::{
    container::{InContainer, InFile, InModule, InSubroutine},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        block::BlockInfo,
        expr::declarator::DeclaratorParent,
        module::{
            Module,
            generate::GenerateBlockId,
            port::{PortDeclId, Ports},
        },
        stmt::StmtKind,
        subroutine::{SubroutineLoc, SubroutinePortId},
    },
    symbol::{DefId, DefLoc, NameScope},
};

fn def_id(db: &dyn HirDb, loc: impl Into<DefLoc>) -> DefId {
    DefId::new(db, loc)
}

impl NameScope {
    pub fn unit_scope_query(db: &dyn HirDb) -> Arc<NameScope> {
        let mut scope = NameScope::default();

        for file_id in db.files().iter() {
            let file_id = HirFileId::File(*file_id);
            let file_scope = db.file_scope(file_id);
            scope.extend_from(&file_scope);
        }

        Arc::new(scope)
    }

    pub(super) fn file_scope_query(db: &dyn HirDb, file_id: HirFileId) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let hir_file = db.hir_file(file_id);

        for (module_id, module_info) in hir_file.modules.iter() {
            scope.insert_value_opt(&module_info.name, def_id(db, InFile::new(file_id, module_id)));
        }

        for (decl_id, decl) in hir_file.decls.iter() {
            scope.insert_value_opt(
                &decl.name,
                def_id(db, InContainer::new(file_id.into(), decl_id)),
            );
        }

        for (config_decl_id, config_decl) in hir_file.config_decls.iter() {
            scope.insert_value_opt(
                &config_decl.name,
                def_id(db, InFile::new(file_id, config_decl_id)),
            );
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

        for (typedef_id, typedef) in hir_file.typedefs.iter() {
            scope.insert_type_opt(
                &typedef.name,
                def_id(db, InContainer::new(file_id.into(), typedef_id)),
            );
        }

        Arc::new(scope)
    }

    pub fn module_scope_query(
        db: &dyn HirDb,
        module_id: crate::hir_def::module::ModuleId,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let (module, module_src_map) = db.module_with_source_map(module_id);
        let file_id = HirFileId::File(module_id.file_id());

        if let Ports::NonAnsi { ports, .. } = &module.ports {
            for (port_id, port) in ports.iter() {
                scope.insert_value_opt(&port.label, def_id(db, InModule::new(module_id, port_id)));
            }
        }

        for (local_subroutine_id, subroutine) in module.subroutines.iter() {
            let Some(src) = module_src_map.get(local_subroutine_id) else {
                continue;
            };
            let subroutine_id = db.intern_subroutine(SubroutineLoc {
                cont_id: module_id.into(),
                src: InFile::new(file_id, src),
                local_id: local_subroutine_id,
            });
            scope.insert_value_opt(&subroutine.name, def_id(db, subroutine_id));
        }

        for (decl_id, decl) in module.decls.iter() {
            scope.insert_value_opt(
                &decl.name,
                def_id(db, InContainer::new(module_id.into(), decl_id)),
            );
        }

        for (typedef_id, typedef) in module.typedefs.iter() {
            scope.insert_type_opt(
                &typedef.name,
                def_id(db, InContainer::new(module_id.into(), typedef_id)),
            );
        }

        for (instance_id, instance) in module.instances.iter() {
            scope.insert_value_opt(
                &instance.name,
                def_id(db, InModule::new(module_id, instance_id)),
            );
        }

        for item in &module_src_map.items {
            if let crate::hir_def::module::ModuleItem::GenerateRegionId(generate_region_id) = item {
                let generate_region = module.get(*generate_region_id);
                for item in &generate_region.items {
                    if let crate::hir_def::module::generate::GenerateItem::GenerateBlockId(
                        generate_block_id,
                    ) = *item
                    {
                        let generate_block = db.generate_block(generate_block_id);
                        scope.insert_value_opt(&generate_block.name, def_id(db, generate_block_id));
                    }
                }
            }
        }

        for (stmt_id, stmt) in module.stmts.iter() {
            scope.insert_value_opt(
                &stmt.label,
                def_id(db, InContainer::new(module_id.into(), stmt_id)),
            );

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_value_opt(name, def_id(db, *block_id));
            }
        }

        Arc::new(scope)
    }

    pub fn generate_block_scope_query(
        db: &dyn HirDb,
        generate_block_id: GenerateBlockId,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let (generate_block, source_map) = db.generate_block_with_source_map(generate_block_id);
        let file_id = HirFileId::File(generate_block_id.file_id(db));

        scope.insert_value_opt(&generate_block.name, def_id(db, generate_block_id));

        for (local_subroutine_id, subroutine) in generate_block.subroutines.iter() {
            let Some(src) = source_map.get(local_subroutine_id) else {
                continue;
            };
            let subroutine_id = db.intern_subroutine(SubroutineLoc {
                cont_id: generate_block_id.into(),
                src: InFile::new(file_id, src),
                local_id: local_subroutine_id,
            });
            scope.insert_value_opt(&subroutine.name, def_id(db, subroutine_id));
        }

        for (decl_id, decl) in generate_block.decls.iter() {
            scope.insert_value_opt(
                &decl.name,
                def_id(db, InContainer::new(generate_block_id.into(), decl_id)),
            );
        }

        for (typedef_id, typedef) in generate_block.typedefs.iter() {
            scope.insert_type_opt(
                &typedef.name,
                def_id(db, InContainer::new(generate_block_id.into(), typedef_id)),
            );
        }

        for item in &generate_block.items {
            if let crate::hir_def::module::generate::GenerateBlockItem::GenerateBlockId(child_id) =
                *item
            {
                let child = db.generate_block(child_id);
                scope.insert_value_opt(&child.name, def_id(db, child_id));
            }
        }

        for (stmt_id, stmt) in generate_block.stmts.iter() {
            scope.insert_value_opt(
                &stmt.label,
                def_id(db, InContainer::new(generate_block_id.into(), stmt_id)),
            );

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_value_opt(name, def_id(db, *block_id));
            }
        }

        Arc::new(scope)
    }

    pub fn block_scope_query(
        db: &dyn HirDb,
        block_id: crate::hir_def::block::BlockId,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let block = db.block(block_id);

        for (decl_id, decl) in block.decls.iter() {
            scope.insert_value_opt(
                &decl.name,
                def_id(db, InContainer::new(block_id.into(), decl_id)),
            );
        }

        for (typedef_id, typedef) in block.typedefs.iter() {
            scope.insert_type_opt(
                &typedef.name,
                def_id(db, InContainer::new(block_id.into(), typedef_id)),
            );
        }

        for (stmt_id, stmt) in block.stmts.iter() {
            scope.insert_value_opt(
                &stmt.label,
                def_id(db, InContainer::new(block_id.into(), stmt_id)),
            );

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_value_opt(name, def_id(db, *block_id));
            }
        }

        Arc::new(scope)
    }

    pub fn subroutine_scope_query(
        db: &dyn HirDb,
        subroutine_id: crate::hir_def::subroutine::SubroutineId,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let subroutine = db.subroutine(subroutine_id);

        for (port_idx, port) in subroutine.ports.iter().enumerate() {
            let port_id = SubroutinePortId(port_idx as u32);
            scope.insert_value_opt(
                &port.name,
                def_id(db, InSubroutine::new(subroutine_id, port_id)),
            );
        }

        for (decl_id, decl) in subroutine.decls.iter() {
            scope.insert_value_opt(
                &decl.name,
                def_id(db, InContainer::new(subroutine_id.into(), decl_id)),
            );
        }

        for (typedef_id, typedef) in subroutine.typedefs.iter() {
            scope.insert_type_opt(
                &typedef.name,
                def_id(db, InContainer::new(subroutine_id.into(), typedef_id)),
            );
        }

        for (stmt_id, stmt) in subroutine.stmts.iter() {
            scope.insert_value_opt(
                &stmt.label,
                def_id(db, InContainer::new(subroutine_id.into(), stmt_id)),
            );

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_value_opt(name, def_id(db, *block_id));
            }
        }

        Arc::new(scope)
    }

    pub fn non_ansi_port_decl_id_by_name(
        &self,
        db: &dyn HirDb,
        module: &Module,
        name: &SmolStr,
    ) -> Option<PortDeclId> {
        let defs = self.values.get(name)?;
        defs.iter().filter_map(|def_id| def_id.as_decl(db)).find_map(|decl_id| {
            let decl = module.get(decl_id.value);
            match decl.parent {
                DeclaratorParent::PortDeclId(port_decl_id) => Some(port_decl_id),
                _ => None,
            }
        })
    }

    fn extend_from(&mut self, other: &NameScope) {
        for (ident, defs) in &other.types {
            for def_id in defs {
                self.insert_type(ident, *def_id);
            }
        }
        for (ident, defs) in &other.values {
            for def_id in defs {
                self.insert_value(ident, *def_id);
            }
        }
        for (ident, defs) in &other.assertions {
            for def_id in defs {
                self.insert_assertion(ident, *def_id);
            }
        }
        self.imports.extend(other.imports.iter().copied());
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
        db::{HirDb, HirDbStorage, InternDbStorage},
        hir_def::Ident,
        symbol::DefKind,
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

    #[test]
    fn name_scope_merged_lookup_covers_current_scope_shapes() {
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
                .lookup_merged(&ident("file_sig"))
                .expect("file decl should be visible")
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Net)
        );
        let shared_defs = unit_scope
            .lookup_merged(&ident("shared"))
            .expect("merged lookup should preserve same-name type and value definitions");
        assert!(shared_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Typedef));
        assert!(shared_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Net));

        let module_id = unit_scope
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");

        let module_scope = db.module_scope(module_id);
        let port_defs =
            module_scope.lookup_merged(&ident("a")).expect("non-ANSI port name should resolve");
        assert!(port_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::NonAnsiPort));
        assert!(port_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Port));
        assert!(port_defs.iter().any(|def_id| def_id.kind(&db) == DefKind::Variable));

        let subroutine_id = module_scope
            .lookup_merged(&ident("f"))
            .and_then(|defs| defs.iter().find_map(|def_id| def_id.as_subroutine(&db)))
            .expect("subroutine should be visible from module scope");
        let subroutine_scope = db.subroutine_scope(subroutine_id);
        assert!(
            subroutine_scope
                .lookup_merged(&ident("p"))
                .expect("subroutine port should be visible")
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::SubroutinePort)
        );

        let block_id = subroutine_scope
            .lookup_merged(&ident("b"))
            .and_then(|defs| defs.iter().find_map(|def_id| def_id.as_block(&db)))
            .expect("named block should be visible from subroutine scope");
        assert!(
            db.block_scope(block_id)
                .lookup_merged(&ident("x"))
                .expect("block local should be visible")
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Variable)
        );

        let generate_block_id = module_scope
            .lookup_merged(&ident("g"))
            .and_then(|defs| defs.iter().find_map(|def_id| def_id.as_generate_block(&db)))
            .expect("generate block should be visible from module scope");
        assert!(
            db.generate_block_scope(generate_block_id)
                .lookup_merged(&ident("y"))
                .expect("generate local should be visible")
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Net)
        );

        // Adding an interface lowering should create a DefKind::Interface
        // producer and insert the resulting DefId into NameScope; IDE
        // feature matches already have default no-op arms.
    }
}
