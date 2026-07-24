use la_arena::{Arena, Idx, RawIdx};
use smol_str::SmolStr;
use syntax::ast;
use triomphe::Arc;
use utils::get::{Get, GetRef};

use crate::{
    container::{InContainer, InFile, InFileOrModule, InModule, InSubroutine, ScopeId},
    db::HirDb,
    def_id::DefId,
    file::HirFileId,
    hir_def::{
        PackageImport,
        block::BlockInfo,
        checker::{CheckerDef, CheckerId, CheckerPortId},
        covergroup::{CovergroupDef, CovergroupId},
        declaration::DeclarationId,
        expr::declarator::{DeclId, Declarator, DeclaratorParent},
        lower_ident_opt,
        module::{
            Module, ModuleKind, PackageId,
            clocking::{ClockingBlockId, ClockingSignalId},
            generate::GenerateBlockId,
            port::{PortDeclId, Ports},
        },
        stmt::{Stmt, StmtKind},
        subroutine::{LocalSubroutineId, SubroutinePortId},
        typedef::{Typedef, TypedefId},
    },
    source_map::ToAstNode,
    symbol::{DefOriginLoc, Import, NameContext, NameScope},
};

// SystemVerilog has separate namespaces. This scope stores current supported
// declarations as:
// - types: modules, interfaces, packages, programs, typedefs
// - values: nets, variables, params, ports, subroutines, instances, blocks
// - assertions: reserved for sequence/property/checker work
// Hierarchical lookup remains a separate resolver path.

fn def_id(db: &dyn HirDb, loc: impl Into<DefOriginLoc>) -> DefId {
    DefId::new(db, loc)
}

/// Inserts the data-net-param declarations and typedefs that every scope
/// owns, keyed by `cont_id`. The declaration/typedef loops are identical across
/// file, module, generate-block, block, and subroutine scopes, so they live
/// here as the single source of truth.
fn insert_decls_and_typedefs(
    scope: &mut NameScope,
    db: &dyn HirDb,
    cont_id: ScopeId,
    decls: &Arena<Declarator>,
    typedefs: &Arena<Typedef>,
) {
    for (decl_id, decl) in decls.iter() {
        scope.insert_value_opt(&decl.name, def_id(db, InContainer::new(cont_id, decl_id)));
    }
    for (typedef_id, typedef) in typedefs.iter() {
        scope.insert_type_opt(&typedef.name, def_id(db, InContainer::new(cont_id, typedef_id)));
    }
}

/// Inserts statement labels (and their nested named blocks), which every
/// scope owns. Kept separate from `insert_decls_and_typedefs` so callers can
/// place module/generate specific members between the two while preserving
/// insertion order.
fn insert_stmts(scope: &mut NameScope, db: &dyn HirDb, cont_id: ScopeId, stmts: &Arena<Stmt>) {
    for (stmt_id, stmt) in stmts.iter() {
        scope.insert_value_opt(&stmt.label, def_id(db, InContainer::new(cont_id, stmt_id)));
        if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
            scope.insert_value_opt(name, def_id(db, *block_id));
        }
    }
}

impl NameScope {
    pub fn unit_scope_query(db: &dyn HirDb) -> Arc<NameScope> {
        let mut scope = NameScope::default();

        for file_id in db.files().iter() {
            let file_id = HirFileId::File(*file_id);
            let file_scope = db.file_scope(file_id);
            scope.extend_defs_from(&file_scope);
        }

        Arc::new(scope)
    }

    pub fn package_export_scope_query(db: &dyn HirDb, package_id: PackageId) -> Arc<NameScope> {
        db.package_export_signature(package_id)
    }

    pub fn package_export_signature_query(db: &dyn HirDb, package_id: PackageId) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let file_id = package_id.file_id;
        let (file, file_source_map) = db.hir_file_with_source_map(file_id);
        if file.get(package_id.value).kind != ModuleKind::Package {
            return Arc::new(scope);
        }

        let tree = db.parse(file_id);
        let Some(package) =
            file_source_map.get(package_id.value).and_then(|src| src.to_node(&tree))
        else {
            return Arc::new(scope);
        };

        let mut builder = PackageExportSignatureBuilder {
            db,
            package_id,
            scope: &mut scope,
            next_declaration: 0,
            next_decl: 0,
            next_typedef: 0,
            next_subroutine: 0,
        };
        builder.collect(package);

        Arc::new(scope)
    }

    pub(super) fn file_scope_query(db: &dyn HirDb, file_id: HirFileId) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let hir_file = db.hir_file(file_id);

        for (module_id, module_info) in hir_file.modules.iter() {
            scope.insert_type_opt(&module_info.name, def_id(db, InFile::new(file_id, module_id)));
        }

        for (_, import) in hir_file.package_imports.iter() {
            scope.insert_package_import(import);
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

        for (checker_id, checker) in hir_file.checkers.iter() {
            scope.insert_type_opt(
                &checker.name,
                def_id(db, InContainer::new(file_id.into(), checker_id)),
            );
        }

        for (covergroup_id, covergroup) in hir_file.covergroups.iter() {
            scope.insert_type_opt(
                &covergroup.name,
                def_id(db, InContainer::new(file_id.into(), covergroup_id)),
            );
        }

        for (coverpoint_id, coverpoint) in hir_file.coverpoints.iter() {
            scope.insert_value_opt(
                &coverpoint.name,
                def_id(db, InContainer::new(file_id.into(), coverpoint_id)),
            );
        }

        for (cross_id, cross) in hir_file.crosses.iter() {
            scope.insert_value_opt(
                &cross.name,
                def_id(db, InContainer::new(file_id.into(), cross_id)),
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

        if let Ports::NonAnsi { ports, .. } = &module.ports {
            for (port_id, port) in ports.iter() {
                scope.insert_value_opt(&port.label, def_id(db, InModule::new(module_id, port_id)));
            }
        }

        for (_, import) in module.package_imports.iter() {
            scope.insert_package_import(import);
        }

        for (local_subroutine_id, subroutine) in module.subroutines.iter() {
            let subroutine_id = InContainer::new(module_id.into(), local_subroutine_id);
            scope.insert_value_opt(&subroutine.name, def_id(db, subroutine_id));
        }

        for (modport_id, modport) in module.modports.iter() {
            scope.insert_value_opt(&modport.name, def_id(db, InModule::new(module_id, modport_id)));
        }

        for (clocking_block_id, clocking_block) in module.clocking_blocks.iter() {
            scope.insert_value_opt(
                &clocking_block.name,
                def_id(db, InModule::new(module_id, clocking_block_id)),
            );
        }

        for (checker_id, checker) in module.checkers.iter() {
            scope.insert_type_opt(
                &checker.name,
                def_id(db, InContainer::new(module_id.into(), checker_id)),
            );
        }

        for (covergroup_id, covergroup) in module.covergroups.iter() {
            scope.insert_type_opt(
                &covergroup.name,
                def_id(db, InContainer::new(module_id.into(), covergroup_id)),
            );
        }

        for (coverpoint_id, coverpoint) in module.coverpoints.iter() {
            scope.insert_value_opt(
                &coverpoint.name,
                def_id(db, InContainer::new(module_id.into(), coverpoint_id)),
            );
        }

        for (cross_id, cross) in module.crosses.iter() {
            scope.insert_value_opt(
                &cross.name,
                def_id(db, InContainer::new(module_id.into(), cross_id)),
            );
        }

        insert_decls_and_typedefs(
            &mut scope,
            db,
            module_id.into(),
            &module.decls,
            &module.typedefs,
        );

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

        insert_stmts(&mut scope, db, module_id.into(), &module.stmts);

        Arc::new(scope)
    }

    pub fn clocking_block_scope_query(
        db: &dyn HirDb,
        clocking_block_id: InModule<ClockingBlockId>,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let module = db.module(clocking_block_id.module_id);
        let clocking_block = module.get(clocking_block_id.value);
        let clocking_scope = ScopeId::ClockingBlock(clocking_block_id);

        for (idx, signal) in clocking_block.signals.iter().enumerate() {
            let signal_id = ClockingSignalId(idx as u32);
            scope.insert_value(
                &signal.name,
                def_id(db, InContainer::new(clocking_scope, signal_id)),
            );
        }

        Arc::new(scope)
    }

    pub fn checker_scope_query(
        db: &dyn HirDb,
        checker_id: InContainer<CheckerId>,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let Some(checker) = checker_def(db, checker_id) else {
            return Arc::new(scope);
        };
        let Ok(checker_scope_id) = InFileOrModule::try_from(checker_id) else {
            return Arc::new(scope);
        };
        let checker_scope = ScopeId::Checker(checker_scope_id);

        for (idx, port) in checker.ports.iter().enumerate() {
            scope.insert_value(
                &port.name,
                def_id(db, InContainer::new(checker_scope, CheckerPortId(idx as u32))),
            );
        }

        let container = checker_id.cont_id.to_container(db);
        for declaration_id in &checker.declarations {
            let declaration = container.declaration(*declaration_id);
            for decl_id in declaration.decls() {
                let decl = container.declarator(decl_id);
                scope.insert_value_opt(
                    &decl.name,
                    def_id(db, InContainer::new(checker_id.cont_id, decl_id)),
                );
            }
        }

        Arc::new(scope)
    }

    pub fn covergroup_scope_query(
        db: &dyn HirDb,
        covergroup_id: InContainer<CovergroupId>,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let Some(covergroup) = covergroup_def(db, covergroup_id) else {
            return Arc::new(scope);
        };
        let Ok(covergroup_scope_id) = InFileOrModule::try_from(covergroup_id) else {
            return Arc::new(scope);
        };
        let covergroup_scope = ScopeId::Covergroup(covergroup_scope_id);

        match covergroup_id.cont_id {
            ScopeId::File(file_id) => {
                let file = db.hir_file(file_id);
                for coverpoint_id in &covergroup.coverpoints {
                    let coverpoint = file.get(*coverpoint_id);
                    scope.insert_value_opt(
                        &coverpoint.name,
                        def_id(db, InContainer::new(covergroup_scope, *coverpoint_id)),
                    );
                }

                for cross_id in &covergroup.crosses {
                    let cross = file.get(*cross_id);
                    scope.insert_value_opt(
                        &cross.name,
                        def_id(db, InContainer::new(covergroup_scope, *cross_id)),
                    );
                }
            }
            ScopeId::Module(module_id) => {
                let module = db.module(module_id);
                for coverpoint_id in &covergroup.coverpoints {
                    let coverpoint = module.get(*coverpoint_id);
                    scope.insert_value_opt(
                        &coverpoint.name,
                        def_id(db, InContainer::new(covergroup_scope, *coverpoint_id)),
                    );
                }

                for cross_id in &covergroup.crosses {
                    let cross = module.get(*cross_id);
                    scope.insert_value_opt(
                        &cross.name,
                        def_id(db, InContainer::new(covergroup_scope, *cross_id)),
                    );
                }
            }
            ScopeId::GenerateBlock(_)
            | ScopeId::Block(_)
            | ScopeId::Subroutine(_)
            | ScopeId::ClockingBlock(_)
            | ScopeId::Checker(_)
            | ScopeId::Covergroup(_) => {}
        }

        Arc::new(scope)
    }

    pub fn generate_block_scope_query(
        db: &dyn HirDb,
        generate_block_id: GenerateBlockId,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let (generate_block, _) = db.generate_block_with_source_map(generate_block_id);

        scope.insert_value_opt(&generate_block.name, def_id(db, generate_block_id));

        for (local_subroutine_id, subroutine) in generate_block.subroutines.iter() {
            let subroutine_id = InContainer::new(generate_block_id.into(), local_subroutine_id);
            scope.insert_value_opt(&subroutine.name, def_id(db, subroutine_id));
        }

        insert_decls_and_typedefs(
            &mut scope,
            db,
            generate_block_id.into(),
            &generate_block.decls,
            &generate_block.typedefs,
        );

        for item in &generate_block.items {
            if let crate::hir_def::module::generate::GenerateBlockItem::GenerateBlockId(child_id) =
                *item
            {
                let child = db.generate_block(child_id);
                scope.insert_value_opt(&child.name, def_id(db, child_id));
            }
        }

        insert_stmts(&mut scope, db, generate_block_id.into(), &generate_block.stmts);

        Arc::new(scope)
    }

    pub fn block_scope_query(
        db: &dyn HirDb,
        block_id: crate::hir_def::block::BlockId,
    ) -> Arc<NameScope> {
        let mut scope = NameScope::default();
        let block = db.block(block_id);

        insert_decls_and_typedefs(&mut scope, db, block_id.into(), &block.decls, &block.typedefs);
        insert_stmts(&mut scope, db, block_id.into(), &block.stmts);

        Arc::new(scope)
    }

    pub fn subroutine_scope_query(
        db: &dyn HirDb,
        subroutine_id: InContainer<LocalSubroutineId>,
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

        insert_decls_and_typedefs(
            &mut scope,
            db,
            subroutine_id.into(),
            &subroutine.decls,
            &subroutine.typedefs,
        );
        insert_stmts(&mut scope, db, subroutine_id.into(), &subroutine.stmts);

        Arc::new(scope)
    }

    pub fn non_ansi_port_decl_id_by_name(
        &self,
        db: &dyn HirDb,
        module: &Module,
        name: &SmolStr,
    ) -> Option<PortDeclId> {
        let def = self.lookup(NameContext::Value, name).unique()?;
        def.origins(db).into_iter().filter_map(|origin| origin.as_decl(db)).find_map(|decl_id| {
            let decl = module.get(decl_id.value);
            match decl.parent {
                DeclaratorParent::PortDeclId(port_decl_id) => Some(port_decl_id),
                _ => None,
            }
        })
    }

    fn insert_package_import(&mut self, import: &PackageImport) {
        self.imports.push(Import { package: import.package.clone(), name: import.item.clone() });
    }

    fn extend_defs_from(&mut self, other: &NameScope) {
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
    }
}

fn checker_def(db: &dyn HirDb, checker_id: InContainer<CheckerId>) -> Option<CheckerDef> {
    match checker_id.cont_id {
        ScopeId::File(file_id) => Some(db.hir_file(file_id).get(checker_id.value).clone()),
        ScopeId::Module(module_id) => Some(db.module(module_id).get(checker_id.value).clone()),
        ScopeId::GenerateBlock(_)
        | ScopeId::Block(_)
        | ScopeId::Subroutine(_)
        | ScopeId::ClockingBlock(_)
        | ScopeId::Checker(_)
        | ScopeId::Covergroup(_) => None,
    }
}

fn covergroup_def(
    db: &dyn HirDb,
    covergroup_id: InContainer<CovergroupId>,
) -> Option<CovergroupDef> {
    match covergroup_id.cont_id {
        ScopeId::File(file_id) => Some(db.hir_file(file_id).get(covergroup_id.value).clone()),
        ScopeId::Module(module_id) => Some(db.module(module_id).get(covergroup_id.value).clone()),
        ScopeId::GenerateBlock(_)
        | ScopeId::Block(_)
        | ScopeId::Subroutine(_)
        | ScopeId::ClockingBlock(_)
        | ScopeId::Checker(_)
        | ScopeId::Covergroup(_) => None,
    }
}

struct PackageExportSignatureBuilder<'a> {
    db: &'a dyn HirDb,
    package_id: PackageId,
    scope: &'a mut NameScope,
    next_declaration: u32,
    next_decl: u32,
    next_typedef: u32,
    next_subroutine: u32,
}

impl PackageExportSignatureBuilder<'_> {
    fn collect(&mut self, package: ast::ModuleDeclaration<'_>) {
        for member in package.members().children() {
            use ast::Member::*;
            match member {
                DataDeclaration(decl) => self.record_declarators(decl.declarators()),
                NetDeclaration(decl) => self.record_declarators(decl.declarators()),
                ParameterDeclarationStatement(decl) => self.record_param_decl(decl.parameter()),
                TypedefDeclaration(decl) => self.record_typedef(decl),
                GenvarDeclaration(decl) => self.record_identifier_names(decl.identifiers()),
                SpecparamDeclaration(decl) => self.record_specparam_declarators(decl.declarators()),
                FunctionDeclaration(decl) => self.record_subroutine(decl),
                _ => {}
            }
        }
    }

    fn record_declarators<'a>(&mut self, declarators: ast::SeparatedList<'a, ast::Declarator<'a>>) {
        let _declaration_id = self.next_declaration_id();
        for declarator in declarators.children() {
            self.record_decl_name(lower_ident_opt(declarator.name()));
        }
    }

    fn record_specparam_declarators<'a>(
        &mut self,
        declarators: ast::SeparatedList<'a, ast::SpecparamDeclarator<'a>>,
    ) {
        let _declaration_id = self.next_declaration_id();
        for declarator in declarators.children() {
            self.record_decl_name(lower_ident_opt(declarator.name()));
        }
    }

    fn record_identifier_names<'a>(
        &mut self,
        identifiers: ast::SeparatedList<'a, ast::IdentifierName<'a>>,
    ) {
        let _declaration_id = self.next_declaration_id();
        for ident in identifiers.children() {
            self.record_decl_name(lower_ident_opt(ident.identifier()));
        }
    }

    fn record_param_decl(&mut self, param_decl: ast::ParameterDeclarationBase<'_>) {
        match param_decl {
            ast::ParameterDeclarationBase::ParameterDeclaration(decl) => {
                self.record_declarators(decl.declarators());
            }
            ast::ParameterDeclarationBase::TypeParameterDeclaration(_) => {
                let _declaration_id = self.next_declaration_id();
            }
        }
    }

    fn record_decl_name(&mut self, name: Option<crate::hir_def::Ident>) {
        let decl_id = self.next_decl_id();
        self.scope.insert_value_opt(
            &name,
            def_id(self.db, InContainer::new(self.package_id.into(), decl_id)),
        );
    }

    fn record_typedef(&mut self, typedef: ast::TypedefDeclaration<'_>) {
        let typedef_id = self.next_typedef_id();
        let name = lower_ident_opt(typedef.name());
        self.scope.insert_type_opt(
            &name,
            def_id(self.db, InContainer::new(self.package_id.into(), typedef_id)),
        );
    }

    fn record_subroutine(&mut self, subroutine: ast::FunctionDeclaration<'_>) {
        let local_id = self.next_subroutine_id();
        let name = lower_name(subroutine.prototype().name());
        self.scope.insert_value_opt(
            &name,
            def_id(self.db, InContainer::new(self.package_id.into(), local_id)),
        );
    }

    fn next_declaration_id(&mut self) -> DeclarationId {
        let id = Idx::from_raw(RawIdx::from(self.next_declaration));
        self.next_declaration += 1;
        id
    }

    fn next_decl_id(&mut self) -> DeclId {
        let id = Idx::from_raw(RawIdx::from(self.next_decl));
        self.next_decl += 1;
        id
    }

    fn next_typedef_id(&mut self) -> TypedefId {
        let id = Idx::from_raw(RawIdx::from(self.next_typedef));
        self.next_typedef += 1;
        id
    }

    fn next_subroutine_id(&mut self) -> LocalSubroutineId {
        let id = Idx::from_raw(RawIdx::from(self.next_subroutine));
        self.next_subroutine += 1;
        id
    }
}

fn lower_name(name: ast::Name<'_>) -> Option<crate::hir_def::Ident> {
    if let Some(id) = name.as_identifier_name().and_then(|name| name.identifier()) {
        return lower_ident_opt(Some(id));
    }
    if let Some(select) = name.as_identifier_select_name() {
        return select.identifier().and_then(|tok| lower_ident_opt(Some(tok)));
    }
    if let Some(scoped) = name.as_scoped_name() {
        return lower_name(scoped.right());
    }
    None
}

#[cfg(test)]
mod tests {
    use std::fmt;

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
        container::{InContainer, ScopeId},
        db::{HirDb, HirDbStorage, InternDbStorage},
        def_id::DefId,
        display::HirDisplay,
        hir_def::{
            Ident,
            module::port::{NonAnsiPortSrc, PortSrcs, Ports},
        },
        semantics::pathres::resolve_name,
        source_map::IsNamedSrc,
        symbol::{DefKind, DefOriginLoc, NameContext, Resolution},
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

        let module_scope = db.module_scope(module_id);
        let port_def = module_scope
            .lookup(NameContext::Value, &ident("a"))
            .unique()
            .expect("non-ANSI port name should resolve uniquely");
        assert_eq!(port_def.kind(&db), DefKind::Port);
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
        let subroutine_scope = db.subroutine_scope(subroutine_id);
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
            db.block_scope(block_id)
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
            db.generate_block_scope(generate_block_id)
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
            .module_scope(module_id)
            .lookup(NameContext::Value, &ident("a"))
            .unique()
            .expect("one logical non-ANSI port should resolve uniquely");
        let origins = port.origins(&db);
        assert_eq!(origins.len(), 3);
        assert_eq!(port.declaration_origin(&db).kind(&db), DefKind::Port);
        for origin in origins {
            assert_eq!(DefId::new(&db, origin.loc(&db)), port);
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
        let (module, source_map) = db.module_with_source_map(module_id);
        let Ports::NonAnsi { ports, .. } = &module.ports else {
            panic!("module should have non-ANSI ports");
        };
        let (port_id, _) = ports.iter().next().expect("port should lower");
        let source = source_map.get(port_id).expect("port should retain its source");

        assert!(source.name_range().is_some(), "explicit port name range should be preserved");
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
        let (module, source_map) = db.module_with_source_map(module_id);
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
        let natural_source = NonAnsiPortSrc::from_ast(TOP.into(), port_ast);
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
            .module_scope(module_id)
            .lookup(NameContext::Value, &ident("a"))
            .unique()
            .expect("port should resolve uniquely");
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
            .module_scope(module_id)
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
            db.module_scope(module_id).lookup(NameContext::Value, &ident("a"))
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
            db.module_scope(module_id).lookup(NameContext::Value, &ident("a"))
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
            db.module_scope(module_id).lookup(NameContext::Value, &ident("a"))
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
        let (module, source_map) = db.module_with_source_map(module_id);
        let (expr_id, expr) =
            module.exprs.iter().next().expect("initializer expression should lower");

        assert_eq!(
            expr,
            &crate::hir_def::expr::Expr::Unsupported(
                syntax::SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION
            ),
            "valid but unsupported syntax must carry an explicit diagnostic kind"
        );
        assert!(
            source_map.get(expr_id).is_some(),
            "valid but unsupported syntax must retain its source"
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
        let (module, source_map) = db.module_with_source_map(module_id);
        let (empty_id, _) = module
            .stmts
            .iter()
            .find(|(_, stmt)| matches!(stmt.kind, crate::hir_def::stmt::StmtKind::Empty))
            .expect("empty statement should lower");
        assert!(source_map.get(empty_id).is_some());

        let mut missing_file = crate::hir_def::file::HirFile::default();
        let mut missing_file_source_map = crate::hir_def::file::FileSourceMap::default();
        let mut ctx = crate::hir_def::lower::LoweringCtx::new(
            &db,
            TOP.into(),
            ScopeId::File(TOP.into()),
            crate::hir_def::lower::FileStore {
                data: &mut missing_file,
                sources: &mut missing_file_source_map,
            },
        );
        let missing_id = ctx.lower_stmt_opt(None);
        drop(ctx);

        assert!(matches!(
            missing_file.stmts[missing_id].kind,
            crate::hir_def::stmt::StmtKind::Missing
        ));
        assert!(missing_file_source_map.stmt_srcs.get(missing_id).is_none());
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
        let module = db.module(module_id);
        let stream = module
            .exprs
            .iter()
            .find_map(|(_, expr)| match expr {
                crate::hir_def::expr::Expr::Stream { concats, .. } => Some(concats),
                _ => None,
            })
            .expect("streaming concatenation should lower");

        assert_eq!(stream.len(), 1);
        assert!(matches!(
            stream[0].with_range.as_ref().and_then(|range| range.selector),
            Some(crate::hir_def::expr::Selector::Range(_, _))
        ));
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

        let module_id = db
            .unit_scope()
            .module_ids(&db, &ident("m"))
            .unique()
            .expect("module should resolve uniquely");
        let module = db.module(module_id);
        let (stream_id, _) = module
            .exprs
            .iter()
            .find(|(_, expr)| matches!(expr, crate::hir_def::expr::Expr::Stream { .. }))
            .expect("streaming concatenation should lower");

        assert_eq!(
            InContainer::new(module_id.into(), stream_id).display_source(&db).unwrap(),
            "{<<{a with [3:0]}}"
        );
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
        let module = db.module(module_id);
        let stream = module
            .exprs
            .iter()
            .find_map(|(_, expr)| match expr {
                crate::hir_def::expr::Expr::Stream { concats, .. } => Some(concats),
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
            Some(crate::hir_def::expr::Selector::Range(_, _))
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
        let (module, source_map) = db.module_with_source_map(module_id);
        let (clocking_block_id, clocking_block) =
            module.clocking_blocks.iter().next().expect("clocking block should lower");
        assert_eq!(clocking_block.name.as_deref(), Some("cb"));
        assert!(matches!(
            module.event_exprs[clocking_block.event],
            crate::hir_def::expr::timing_control::EventExpr::Atom {
                sensitivity: Some(crate::hir_def::expr::timing_control::Sensitivity::Posedge),
                ..
            }
        ));
        assert!(source_map.get(clocking_block.event).is_some());
        assert_eq!(
            module.default_clocking.as_ref().and_then(|reference| reference.name.as_deref()),
            Some("cb")
        );
        assert!(source_map.default_clocking_src.is_some());
        assert_eq!(clocking_block.signals.len(), 1);
        assert_eq!(clocking_block.signals[0].name.as_str(), "a");

        let defs = db.module_scope(module_id).lookup(NameContext::Value, &ident("cb"));
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
            .copied()
            .find_map(|def_id| def_id.primary_origin(&db).as_checker(&db))
            .expect("checker definition should have a concrete id");
        let checker_scope = db.checker_scope(checker_id);
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
        let module = db.module(module_id);
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
        let module = db.module(module_id);
        let (covergroup_id, covergroup) =
            module.covergroups.iter().next().expect("covergroup should lower");
        assert_eq!(covergroup.name.as_deref(), Some("cg"));
        assert_eq!(covergroup.coverpoints.len(), 1);
        assert_eq!(covergroup.crosses.len(), 1);

        let coverpoint_id = covergroup.coverpoints[0];
        let cross_id = covergroup.crosses[0];
        assert_eq!(module.get(coverpoint_id).name.as_deref(), Some("cp"));
        assert_eq!(module.get(cross_id).name.as_deref(), Some("cx"));

        let module_scope = db.module_scope(module_id);
        let covergroup_defs = module_scope.lookup(NameContext::Type, &ident("cg"));
        assert!(covergroup_defs.iter().any(|def_id| {
            def_id.kind(&db) == DefKind::Covergroup
                && def_id
                    .primary_origin(&db)
                    .as_covergroup(&db)
                    .is_some_and(|id| id.value == covergroup_id)
        }));

        let coverpoint_defs = module_scope.lookup(NameContext::Value, &ident("cp"));
        assert!(coverpoint_defs.iter().any(|def_id| {
            matches!(def_id.primary_origin(&db).loc(&db), DefOriginLoc::Coverpoint(id) if id.value == coverpoint_id)
        }));

        let cross_defs = module_scope.lookup(NameContext::Value, &ident("cx"));
        assert!(
            cross_defs
                .iter()
                .any(|def_id| matches!(def_id.primary_origin(&db).loc(&db), DefOriginLoc::Cross(id) if id.value == cross_id))
        );

        let covergroup_scope =
            db.covergroup_scope(InContainer::new(module_id.into(), covergroup_id));
        let scoped_coverpoint_defs = covergroup_scope.lookup(NameContext::Value, &ident("cp"));
        assert!(scoped_coverpoint_defs.iter().any(|def_id| {
            matches!(def_id.primary_origin(&db).loc(&db), DefOriginLoc::Coverpoint(id) if matches!(id.cont_id, ScopeId::Covergroup(_)) && id.value == coverpoint_id)
        }));
        let scoped_cross_defs = covergroup_scope.lookup(NameContext::Value, &ident("cx"));
        assert!(scoped_cross_defs.iter().any(|def_id| {
            matches!(def_id.primary_origin(&db).loc(&db), DefOriginLoc::Cross(id) if matches!(id.cont_id, ScopeId::Covergroup(_)) && id.value == cross_id)
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
        let package_exports = db.package_export_scope(package_id);
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
        let wildcard_scope = db.module_scope(wildcard_importer);
        assert!(
            wildcard_scope
                .imports
                .iter()
                .any(|import| import.package == ident("pkg") && import.name.is_none())
        );

        let imported_t = resolve_name(
            &db,
            ScopeId::Module(wildcard_importer),
            &ident("imported_t"),
            NameContext::Type,
        );
        assert!(imported_t.iter().any(|def_id| def_id.kind(&db) == DefKind::Typedef));
        assert!(
            resolve_name(
                &db,
                ScopeId::Module(wildcard_importer),
                &ident("imported_t"),
                NameContext::Value,
            )
            .is_unresolved(),
            "value lookup should not fall back to the type bucket"
        );

        let shadowed_v = resolve_name(
            &db,
            ScopeId::Module(wildcard_importer),
            &ident("shadowed_v"),
            NameContext::Value,
        );
        assert!(shadowed_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Net));
        assert!(!shadowed_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Variable));

        let named_importer = db
            .unit_scope()
            .module_ids(&db, &ident("named_importer"))
            .unique()
            .expect("named importer should resolve uniquely");
        let named_scope = db.module_scope(named_importer);
        assert!(named_scope.imports.iter().any(|import| {
            import.package == ident("pkg")
                && import.name.as_ref().is_some_and(|name| name == "imported_v")
        }));

        let imported_v = resolve_name(
            &db,
            ScopeId::Module(named_importer),
            &ident("imported_v"),
            NameContext::Value,
        );
        assert!(imported_v.iter().any(|def_id| def_id.kind(&db) == DefKind::Variable));
        assert!(
            resolve_name(
                &db,
                ScopeId::Module(named_importer),
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

        let package_id = db
            .unit_scope()
            .package_ids(&db, &ident("pkg"))
            .unique()
            .expect("package should resolve uniquely");
        let package_f =
            resolve_name(&db, ScopeId::Module(package_id), &ident("f"), NameContext::Value)
                .unique()
                .expect("package scope should resolve package subroutine");

        let DefOriginLoc::Subroutine(package_subroutine) = package_f.primary_origin(&db).loc(&db)
        else {
            panic!("package f should resolve to a subroutine");
        };
        assert_eq!(package_subroutine.cont_id, ScopeId::Module(package_id));

        let named_importer = db
            .unit_scope()
            .module_ids(&db, &ident("named_importer"))
            .unique()
            .expect("named importer should resolve uniquely");
        let named_import_f =
            resolve_name(&db, ScopeId::Module(named_importer), &ident("f"), NameContext::Value)
                .unique()
                .expect("named import should resolve package subroutine");

        let wildcard_importer = db
            .unit_scope()
            .module_ids(&db, &ident("wildcard_importer"))
            .unique()
            .expect("wildcard importer should resolve uniquely");
        let wildcard_import_f =
            resolve_name(&db, ScopeId::Module(wildcard_importer), &ident("f"), NameContext::Value)
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

        let exports = db.package_export_scope(package_id);
        assert!(
            exports
                .lookup(NameContext::Value, &ident("exported_f"))
                .iter()
                .any(|def_id| def_id.kind(&db) == DefKind::Subroutine)
        );

        let before_body_edit = db.package_export_signature(package_id);
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
    }
}
