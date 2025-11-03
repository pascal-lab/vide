use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::{define_enum_deriving_from, get::GetRef};

use crate::{
    completion::{CompletionEntry, CompletionEntryKind},
    container::{ContainerId, InContainer, InFile, InModule, InPackage},
    db::HirDb,
    display::HirDisplay,
    file::HirFileId,
    hir_def::{
        Ident,
        aggregate::{ClassId, StructId},
        block::{Block, BlockId, BlockInfo},
        declaration::Declaration,
        expr::{
            data_ty::DataTy,
            declarator::{DeclId, DeclaratorParent},
        },
        module::{
            Module, ModuleId,
            instantiation::InstanceId,
            port::{NonAnsiPortId, PortDirection, PortHeader, Ports},
        },
        package::{Package, PackageExport, PackageId, PackageImportId, PackageImportMember},
        proc::ProcId,
        stmt::{StmtId, StmtKind},
        subroutine::{Subroutine, SubroutineId, SubroutineKind},
        ty::NetKind,
        typedef::{Typedef, TypedefId},
    },
};

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum UnitEntry {
        ModuleId(ModuleId),
        PackageId(PackageId),
        FiledDeclId(FiledDeclId),
        TypedefId(InFile<TypedefId>),
        ClassId(InFile<ClassId>),
    }
}

pub type FiledDeclId = InFile<DeclId>;

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum ModuleEntry {
        DeclId(DeclId),
        NonAnsiPortEntry(NonAnsiPortEntry),
        AnsiPortEntry(AnsiPortEntry),
        InstanceId(InstanceId),
        StmtId(StmtId),
        BlockId(BlockId),
        TypedefId(TypedefId),
        ClassId(ClassId),
        PackageImportEntry(PackageImportEntry),
        PackageMember(PackageEntry),
        SubroutineId(SubroutineId),
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct NonAnsiPortEntry {
    // explicit label for port
    pub label: Option<NonAnsiPortId>,
    pub port_decl: Option<DeclId>,
    pub data_decl: Option<DeclId>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct AnsiPortEntry(pub DeclId);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct PackageImportEntry {
    pub import: PackageImportId,
    pub item_idx: u32,
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum BlockEntry {
        StmtId,
        DeclId,
        BlockId,
        TypedefId,
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum SubroutineEntry {
        StmtId,
        DeclId,
        BlockId,
        TypedefId,
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum PackageEntry {
        DeclId(InPackage<DeclId>),
        TypedefId(InPackage<TypedefId>),
        ClassId(InPackage<ClassId>),
        StructId(InPackage<StructId>),
        ProcId(InPackage<ProcId>),
        SubroutineId(InPackage<SubroutineId>),
        Package(InPackage<PackageId>),
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Scope<Entry> {
    entries: FxHashMap<Ident, Entry>,
}

impl<Entry> Default for Scope<Entry> {
    fn default() -> Self {
        Scope { entries: FxHashMap::default() }
    }
}

impl<Entry: Copy> Scope<Entry> {
    pub(crate) fn insert(&mut self, ident: &Ident, entry: Entry) -> Option<Entry> {
        self.entries.insert(ident.clone(), entry)
    }

    pub(crate) fn insert_opt(&mut self, ident: &Option<Ident>, entry: Entry) -> Option<Entry> {
        self.insert(ident.as_ref()?, entry)
    }

    pub fn get(&self, ident: &Ident) -> Option<Entry> {
        self.entries.get(ident).copied()
    }

    pub(crate) fn get_mut(&mut self, ident: &Ident) -> Option<&mut Entry> {
        self.entries.get_mut(ident)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Ident, &Entry)> {
        self.entries.iter()
    }
}

pub type UnitScope = Scope<UnitEntry>;
pub type ModuleScope = Scope<ModuleEntry>;
pub type PackageScope = Scope<PackageEntry>;
pub type BlockScope = Scope<BlockEntry>;
pub type SubroutineScope = Scope<SubroutineEntry>;

// TODO: diagnostics

impl UnitScope {
    pub fn unit_scope_query(db: &dyn HirDb) -> Arc<UnitScope> {
        let mut scope = Scope::default();

        for file_id in db.files().iter() {
            let file_id = HirFileId(*file_id);
            let file_scope = db.file_scope(file_id);
            scope.entries.extend(file_scope.entries.clone().into_iter());
        }

        Arc::new(scope)
    }

    pub(super) fn file_scope_query(db: &dyn HirDb, file_id: HirFileId) -> Arc<UnitScope> {
        let mut scope = Scope::default();
        let hir_file = db.hir_file(file_id);

        for (module_id, module_info) in hir_file.modules.iter() {
            scope.insert_opt(&module_info.name, InFile::new(file_id, module_id).into());
        }

        for (package_id, package_info) in hir_file.packages.iter() {
            scope.insert_opt(&package_info.name, InFile::new(file_id, package_id).into());
        }

        for (decl_id, decl) in hir_file.decls.iter() {
            scope.insert_opt(&decl.name, InFile::new(file_id, decl_id).into());
        }

        for (typedef_id, typedef_) in hir_file.typedefs.iter() {
            scope.insert_opt(&typedef_.name, InFile::new(file_id, typedef_id).into());
        }

        for (class_id, class_) in hir_file.classes.iter() {
            scope.insert_opt(&class_.name, InFile::new(file_id, class_id).into());
        }

        Arc::new(scope)
    }

    pub fn collect_completions(&self, db: &dyn HirDb) -> Vec<CompletionEntry> {
        let mut items = Vec::new();

        for (ident, entry) in self.iter() {
            match entry {
                UnitEntry::ModuleId(_) => {
                    items.push(make_entry(ident, CompletionEntryKind::Module));
                }
                UnitEntry::PackageId(_) => {
                    items.push(make_entry(ident, CompletionEntryKind::Module));
                }
                UnitEntry::FiledDeclId(in_file) => {
                    let file = db.hir_file(in_file.file_id);
                    let declarator = file.decls.get(in_file.value);
                    let kind = match declarator.parent {
                        DeclaratorParent::PortDeclId(_) => CompletionEntryKind::Port,
                        DeclaratorParent::DeclarationId(declaration_id) => {
                            match file.declarations.get(declaration_id) {
                                Declaration::ParamDecl(_) => CompletionEntryKind::Parameter,
                                Declaration::NetDecl(_) => CompletionEntryKind::Net,
                                Declaration::DataDecl(_) => CompletionEntryKind::Variable,
                            }
                        }
                        DeclaratorParent::StmtId(_) => CompletionEntryKind::Variable,
                    };
                    let detail = file_decl_detail(db, in_file.file_id, &file, in_file.value);
                    items.push(make_entry_with_detail(ident, kind, detail));
                }
                UnitEntry::TypedefId(in_file) => {
                    let file = db.hir_file(in_file.file_id);
                    let typedef = file.typedefs.get(in_file.value);
                    let detail =
                        typedef_detail(db, ContainerId::HirFileId(in_file.file_id), typedef);
                    items.push(make_entry_with_detail(ident, CompletionEntryKind::Type, detail));
                }
                UnitEntry::ClassId(_) => {
                    items.push(make_entry(ident, CompletionEntryKind::Type));
                }
            }
        }

        items
    }
}

impl ModuleScope {
    pub fn module_scope_query(db: &dyn HirDb, module_id: ModuleId) -> Arc<ModuleScope> {
        let mut scope = Scope::default();
        let module = db.module(module_id);

        // handle labels of non-ansi ports
        if let Ports::NonAnsi { ports, .. } = &module.ports {
            for (port_id, port) in ports.iter() {
                let entry = NonAnsiPortEntry { label: Some(port_id), ..Default::default() }.into();
                scope.insert_opt(&port.label, entry);
            }
        }

        // handle other members
        for (decl_id, decl) in module.decls.iter() {
            let Some(name) = &decl.name else {
                continue;
            };

            let is_port_decl = matches!(decl.parent, DeclaratorParent::PortDeclId(_));

            if let Some(ModuleEntry::NonAnsiPortEntry(entry)) = scope.get_mut(name) {
                if is_port_decl {
                    entry.port_decl = Some(decl_id);
                } else {
                    entry.data_decl = Some(decl_id);
                }
                continue;
            }

            let entry = if is_port_decl {
                match module.ports {
                    Ports::NonAnsi { .. } => {
                        NonAnsiPortEntry { port_decl: Some(decl_id), ..Default::default() }.into()
                    }
                    Ports::Ansi(_) => AnsiPortEntry(decl_id).into(),
                }
            } else {
                decl_id.into()
            };

            scope.insert(name, entry);
        }

        for (instance_id, instance) in module.instances.iter() {
            scope.insert_opt(&instance.name, instance_id.into());
        }

        for (stmt_id, stmt) in module.stmts.iter() {
            scope.insert_opt(&stmt.label, stmt_id.into());

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_opt(name, (*block_id).into());
            }
        }

        for (typedef_id, typedef_) in module.typedefs.iter() {
            scope.insert_opt(&typedef_.name, typedef_id.into());
        }

        for (class_id, class_) in module.classes.iter() {
            scope.insert_opt(&class_.name, class_id.into());
        }

        for (sub_id, subroutine) in module.subroutines.iter() {
            scope.insert_opt(&subroutine.name, sub_id.into());
        }

        let packages_by_name = db.packages_by_name();
        let mut package_scope_cache: FxHashMap<PackageId, Arc<PackageScope>> = FxHashMap::default();
        let mut get_package_scope = |pkg_id: PackageId| {
            package_scope_cache.entry(pkg_id).or_insert_with(|| db.package_scope(pkg_id)).clone()
        };

        for (import_id, import) in module.package_imports.iter() {
            for (idx, item) in import.items.iter().enumerate() {
                let entry = PackageImportEntry { import: import_id, item_idx: idx as u32 };
                let entry_name: Ident = match &item.member {
                    PackageImportMember::Named(name) => name.clone(),
                    PackageImportMember::All => {
                        SmolStr::from(format!("{}::*", item.package.as_str()))
                    }
                };
                scope.insert(&entry_name, entry.into());

                if let Some(target_packages) = packages_by_name.get(&item.package) {
                    for target_pkg in target_packages {
                        match &item.member {
                            PackageImportMember::All => {
                                let pkg_scope = get_package_scope(*target_pkg);
                                for (target_ident, &pkg_entry) in pkg_scope.iter() {
                                    if scope.get(target_ident).is_none() {
                                        scope.insert(target_ident, pkg_entry.into());
                                    }
                                }
                            }
                            PackageImportMember::Named(name) => {
                                if scope.get(name).is_none() {
                                    let pkg_scope = get_package_scope(*target_pkg);
                                    if let Some(pkg_entry) = pkg_scope.get(name) {
                                        scope.insert(name, pkg_entry.into());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Arc::new(scope)
    }

    pub fn collect_completions(&self, db: &dyn HirDb, module_id: ModuleId) -> Vec<CompletionEntry> {
        let module = db.module(module_id);
        let mut package_cache: FxHashMap<PackageId, Arc<Package>> = FxHashMap::default();
        let mut get_package = |pkg_id: PackageId| {
            package_cache.entry(pkg_id).or_insert_with(|| db.package(pkg_id)).clone()
        };
        let mut items = Vec::new();

        for (ident, entry) in self.iter() {
            let completion = match entry {
                ModuleEntry::DeclId(decl_id) => {
                    let declarator = module.decls.get(*decl_id);
                    let kind = match declarator.parent {
                        DeclaratorParent::PortDeclId(_) => CompletionEntryKind::Port,
                        DeclaratorParent::DeclarationId(declaration_id) => {
                            match module.declarations.get(declaration_id) {
                                Declaration::ParamDecl(_) => CompletionEntryKind::Parameter,
                                Declaration::NetDecl(_) => CompletionEntryKind::Net,
                                Declaration::DataDecl(_) => CompletionEntryKind::Variable,
                            }
                        }
                        DeclaratorParent::StmtId(_) => CompletionEntryKind::Variable,
                    };
                    let detail = module_decl_detail(db, module_id, &module, *decl_id);
                    Some(make_entry_with_detail(ident, kind, detail))
                }
                ModuleEntry::NonAnsiPortEntry(port_entry) => {
                    let detail = non_ansi_port_detail(db, module_id, &module, port_entry);
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Port, detail))
                }
                ModuleEntry::AnsiPortEntry(AnsiPortEntry(decl_id)) => {
                    let detail = module_port_detail(db, module_id, &module, *decl_id);
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Port, detail))
                }
                ModuleEntry::InstanceId(_) => {
                    Some(make_entry(ident, CompletionEntryKind::Instance))
                }
                ModuleEntry::StmtId(_) => Some(make_entry(ident, CompletionEntryKind::Statement)),
                ModuleEntry::BlockId(_) => Some(make_entry(ident, CompletionEntryKind::Block)),
                ModuleEntry::TypedefId(typedef_id) => {
                    let typedef = module.typedefs.get(*typedef_id);
                    let detail = typedef_detail(db, ContainerId::ModuleId(module_id), typedef);
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Type, detail))
                }
                ModuleEntry::ClassId(_) => Some(make_entry(ident, CompletionEntryKind::Type)),
                ModuleEntry::PackageImportEntry(entry) => {
                    let import = module.package_imports.get(entry.import);
                    if let Some(item) = import.items.get(entry.item_idx as usize) {
                        let detail = match &item.member {
                            PackageImportMember::Named(name) => {
                                format!("import {}::{}", item.package, name)
                            }
                            PackageImportMember::All => {
                                format!("import {}::*", item.package)
                            }
                        };
                        Some(make_entry_with_detail(
                            ident,
                            CompletionEntryKind::Import,
                            Some(detail),
                        ))
                    } else {
                        Some(make_entry(ident, CompletionEntryKind::Import))
                    }
                }
                ModuleEntry::SubroutineId(sub_id) => {
                    let sub = module.subroutines.get(*sub_id);
                    let detail = match sub.kind {
                        SubroutineKind::Task => "task".to_string(),
                        SubroutineKind::Function { .. } => "function".to_string(),
                    };
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Function, Some(detail)))
                }
                ModuleEntry::PackageMember(pkg_entry) => match pkg_entry {
                    PackageEntry::DeclId(in_pkg_decl) => {
                        let package = get_package(in_pkg_decl.package_id);
                        let declarator = package.decls.get(in_pkg_decl.value);
                        let (kind, base_detail) = match declarator.parent {
                            DeclaratorParent::DeclarationId(declaration_id) => {
                                let declaration = package.declarations.get(declaration_id);
                                let kind = match declaration {
                                    Declaration::ParamDecl(_) => CompletionEntryKind::Parameter,
                                    Declaration::NetDecl(_) => CompletionEntryKind::Net,
                                    Declaration::DataDecl(_) => CompletionEntryKind::Variable,
                                };
                                let detail = declaration_detail(
                                    db,
                                    ContainerId::PackageId(in_pkg_decl.package_id),
                                    declaration,
                                );
                                (kind, detail)
                            }
                            DeclaratorParent::StmtId(_) | DeclaratorParent::PortDeclId(_) => {
                                (CompletionEntryKind::Variable, None)
                            }
                        };
                        Some(make_entry_with_detail(ident, kind, base_detail))
                    }
                    PackageEntry::TypedefId(in_pkg_typedef) => {
                        let package = get_package(in_pkg_typedef.package_id);
                        let typedef = package.typedefs.get(in_pkg_typedef.value);
                        let detail = typedef_detail(
                            db,
                            ContainerId::PackageId(in_pkg_typedef.package_id),
                            typedef,
                        );
                        Some(make_entry_with_detail(ident, CompletionEntryKind::Type, detail))
                    }
                    PackageEntry::ClassId(_) => Some(make_entry(ident, CompletionEntryKind::Type)),
                    PackageEntry::StructId(_) => Some(make_entry(ident, CompletionEntryKind::Type)),
                    PackageEntry::ProcId(_) => {
                        Some(make_entry(ident, CompletionEntryKind::Function))
                    }
                    PackageEntry::SubroutineId(in_pkg_sub) => {
                        let package = get_package(in_pkg_sub.package_id);
                        let sub = package.subroutines.get(in_pkg_sub.value);
                        let detail = match sub.kind {
                            SubroutineKind::Task => "task".to_string(),
                            SubroutineKind::Function { .. } => "function".to_string(),
                        };
                        Some(make_entry_with_detail(
                            ident,
                            CompletionEntryKind::Function,
                            Some(detail),
                        ))
                    }
                    PackageEntry::Package(_) => Some(make_entry_with_detail(
                        ident,
                        CompletionEntryKind::Module,
                        Some(String::from("package")),
                    )),
                },
            };

            if let Some(entry) = completion {
                items.push(entry);
            }
        }

        items
    }
}

impl BlockScope {
    pub fn block_scope_query(db: &dyn HirDb, block_id: BlockId) -> Arc<BlockScope> {
        let mut scope = Scope::default();
        let block = db.block(block_id);

        for (decl_id, decl) in block.decls.iter() {
            scope.insert_opt(&decl.name, decl_id.into());
        }

        for (stmt_id, stmt) in block.stmts.iter() {
            scope.insert_opt(&stmt.label, stmt_id.into());

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_opt(name, (*block_id).into());
            }
        }

        for (typedef_id, typedef_) in block.typedefs.iter() {
            scope.insert_opt(&typedef_.name, typedef_id.into());
        }

        Arc::new(scope)
    }

    pub fn collect_completions(&self, db: &dyn HirDb, block_id: BlockId) -> Vec<CompletionEntry> {
        let block = db.block(block_id);
        let mut items = Vec::new();

        for (ident, entry) in self.iter() {
            let completion = match entry {
                BlockEntry::DeclId(decl_id) => {
                    let declarator = block.decls.get(*decl_id);
                    let kind = match declarator.parent {
                        DeclaratorParent::PortDeclId(_) => CompletionEntryKind::Port,
                        DeclaratorParent::DeclarationId(declaration_id) => {
                            match block.declarations.get(declaration_id) {
                                Declaration::ParamDecl(_) => CompletionEntryKind::Parameter,
                                Declaration::NetDecl(_) => CompletionEntryKind::Net,
                                Declaration::DataDecl(_) => CompletionEntryKind::Variable,
                            }
                        }
                        DeclaratorParent::StmtId(_) => CompletionEntryKind::Variable,
                    };
                    let detail = block_decl_detail(db, block_id, &block, *decl_id);
                    Some(make_entry_with_detail(ident, kind, detail))
                }
                BlockEntry::StmtId(_) => Some(make_entry(ident, CompletionEntryKind::Statement)),
                BlockEntry::BlockId(_) => Some(make_entry(ident, CompletionEntryKind::Block)),
                BlockEntry::TypedefId(typedef_id) => {
                    let typedef = block.typedefs.get(*typedef_id);
                    let detail = typedef_detail(db, ContainerId::BlockId(block_id), typedef);
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Type, detail))
                }
            };

            if let Some(entry) = completion {
                items.push(entry);
            }
        }

        items
    }
}

impl SubroutineScope {
    pub fn subroutine_scope_query(
        db: &dyn HirDb,
        subroutine_loc: InModule<SubroutineId>,
    ) -> Arc<SubroutineScope> {
        let mut scope = Scope::default();
        let subroutine = db.subroutine(subroutine_loc);

        if !subroutine.has_body {
            return Arc::new(scope);
        }

        for (decl_id, decl) in subroutine.decls.iter() {
            scope.insert_opt(&decl.name, decl_id.into());
        }

        for (stmt_id, stmt) in subroutine.stmts.iter() {
            scope.insert_opt(&stmt.label, stmt_id.into());

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_opt(name, (*block_id).into());
            }
        }

        for (typedef_id, typedef_) in subroutine.typedefs.iter() {
            scope.insert_opt(&typedef_.name, typedef_id.into());
        }

        Arc::new(scope)
    }

    pub fn collect_completions(
        &self,
        db: &dyn HirDb,
        subroutine_loc: InModule<SubroutineId>,
    ) -> Vec<CompletionEntry> {
        let subroutine = db.subroutine(subroutine_loc);
        let mut items = Vec::new();

        for (ident, entry) in self.iter() {
            let completion = match entry {
                SubroutineEntry::DeclId(decl_id) => {
                    let declarator = subroutine.decls.get(*decl_id);
                    let kind = match declarator.parent {
                        DeclaratorParent::PortDeclId(_) => CompletionEntryKind::Port,
                        DeclaratorParent::DeclarationId(declaration_id) => {
                            match subroutine.declarations.get(declaration_id) {
                                Declaration::ParamDecl(_) => CompletionEntryKind::Parameter,
                                Declaration::NetDecl(_) => CompletionEntryKind::Net,
                                Declaration::DataDecl(_) => CompletionEntryKind::Variable,
                            }
                        }
                        DeclaratorParent::StmtId(_) => CompletionEntryKind::Variable,
                    };
                    let detail = subroutine_decl_detail(db, subroutine_loc, &subroutine, *decl_id);
                    Some(make_entry_with_detail(ident, kind, detail))
                }
                SubroutineEntry::StmtId(_) => {
                    Some(make_entry(ident, CompletionEntryKind::Statement))
                }
                SubroutineEntry::BlockId(_) => Some(make_entry(ident, CompletionEntryKind::Block)),
                SubroutineEntry::TypedefId(typedef_id) => {
                    let typedef = subroutine.typedefs.get(*typedef_id);
                    let detail =
                        typedef_detail(db, ContainerId::SubroutineId(subroutine_loc), typedef);
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Type, detail))
                }
            };

            if let Some(entry) = completion {
                items.push(entry);
            }
        }

        items
    }

    pub fn file_subroutine_scope_query(
        db: &dyn HirDb,
        subroutine_loc: InFile<SubroutineId>,
    ) -> Arc<SubroutineScope> {
        let mut scope = Scope::default();
        let subroutine = subroutine_loc.to_container(db);

        if !subroutine.has_body {
            return Arc::new(scope);
        }

        for (decl_id, decl) in subroutine.decls.iter() {
            scope.insert_opt(&decl.name, decl_id.into());
        }

        for (stmt_id, stmt) in subroutine.stmts.iter() {
            scope.insert_opt(&stmt.label, stmt_id.into());

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_opt(name, (*block_id).into());
            }
        }

        for (typedef_id, typedef_) in subroutine.typedefs.iter() {
            scope.insert_opt(&typedef_.name, typedef_id.into());
        }

        Arc::new(scope)
    }

    pub fn collect_file_subroutine_completions(
        &self,
        db: &dyn HirDb,
        subroutine_loc: InFile<SubroutineId>,
    ) -> Vec<CompletionEntry> {
        let subroutine = subroutine_loc.to_container(db);
        let mut items = Vec::new();

        for (ident, entry) in self.iter() {
            let completion = match entry {
                SubroutineEntry::DeclId(decl_id) => {
                    let declarator = subroutine.decls.get(*decl_id);
                    let kind = match declarator.parent {
                        DeclaratorParent::PortDeclId(_) => CompletionEntryKind::Port,
                        DeclaratorParent::DeclarationId(declaration_id) => {
                            match subroutine.declarations.get(declaration_id) {
                                Declaration::ParamDecl(_) => CompletionEntryKind::Parameter,
                                Declaration::NetDecl(_) => CompletionEntryKind::Net,
                                Declaration::DataDecl(_) => CompletionEntryKind::Variable,
                            }
                        }
                        DeclaratorParent::StmtId(_) => CompletionEntryKind::Variable,
                    };
                    let detail = file_subroutine_decl_detail(db, subroutine_loc, &subroutine, *decl_id);
                    Some(make_entry_with_detail(ident, kind, detail))
                }
                SubroutineEntry::StmtId(_) => {
                    Some(make_entry(ident, CompletionEntryKind::Statement))
                }
                SubroutineEntry::BlockId(_) => Some(make_entry(ident, CompletionEntryKind::Block)),
                SubroutineEntry::TypedefId(typedef_id) => {
                    let typedef = subroutine.typedefs.get(*typedef_id);
                    let detail =
                        typedef_detail(db, ContainerId::FileSubroutineId(subroutine_loc), typedef);
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Type, detail))
                }
            };

            if let Some(entry) = completion {
                items.push(entry);
            }
        }

        items
    }
}

fn make_entry(ident: &Ident, kind: CompletionEntryKind) -> CompletionEntry {
    make_entry_with_detail(ident, kind, None)
}

fn make_entry_with_detail(
    ident: &Ident,
    kind: CompletionEntryKind,
    detail: Option<String>,
) -> CompletionEntry {
    match detail {
        Some(detail) => CompletionEntry::new(ident.clone(), kind).with_detail(detail),
        None => CompletionEntry::new(ident.clone(), kind).with_detail(kind.as_str()),
    }
}

fn data_ty_signature(db: &dyn HirDb, container_id: ContainerId, ty: DataTy) -> Option<String> {
    InContainer::new(container_id, ty).display_signature(db).ok()
}

fn typedef_detail(db: &dyn HirDb, container_id: ContainerId, typedef: &Typedef) -> Option<String> {
    typedef.ty.and_then(|ty| data_ty_signature(db, container_id, ty))
}

fn port_direction_str(dir: PortDirection) -> &'static str {
    match dir {
        PortDirection::Input => "input",
        PortDirection::Output => "output",
        PortDirection::Ref => "ref",
        PortDirection::Inout => "inout",
    }
}

fn net_kind_str(kind: NetKind) -> &'static str {
    match kind {
        NetKind::Supply0 => "supply0",
        NetKind::Supply1 => "supply1",
        NetKind::Tri => "tri",
        NetKind::Triand => "triand",
        NetKind::Trior => "trior",
        NetKind::Tri0 => "tri0",
        NetKind::Tri1 => "tri1",
        NetKind::Wire => "wire",
        NetKind::Wand => "wand",
        NetKind::Wor => "wor",
        NetKind::Uwire => "uwire",
    }
}

fn port_header_detail(db: &dyn HirDb, module_id: ModuleId, header: &PortHeader) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(dir) = header.dir() {
        parts.push(port_direction_str(dir).to_string());
    }

    match header {
        PortHeader::Var { var_kw, ty, .. } => {
            if *var_kw {
                parts.push("var".to_string());
            }
            if let Some(sig) = data_ty_signature(db, ContainerId::ModuleId(module_id), *ty) {
                parts.push(sig);
            }
        }
        PortHeader::Net { net_ty, .. } => {
            parts.push(net_kind_str(net_ty.kind).to_string());
            if let Some(sig) = data_ty_signature(db, ContainerId::ModuleId(module_id), net_ty.ty) {
                parts.push(sig);
            }
        }
    }

    if parts.is_empty() { None } else { Some(parts.join(" ")) }
}

fn module_port_detail(
    db: &dyn HirDb,
    module_id: ModuleId,
    module: &Module,
    decl_id: DeclId,
) -> Option<String> {
    let declarator = module.decls.get(decl_id);
    let DeclaratorParent::PortDeclId(port_decl_id) = declarator.parent else {
        return None;
    };
    let port_decl = module.ports.get(port_decl_id);
    port_header_detail(db, module_id, &port_decl.header)
}

fn module_decl_detail(
    db: &dyn HirDb,
    module_id: ModuleId,
    module: &Module,
    decl_id: DeclId,
) -> Option<String> {
    let declarator = module.decls.get(decl_id);
    match declarator.parent {
        DeclaratorParent::PortDeclId(_) => module_port_detail(db, module_id, module, decl_id),
        DeclaratorParent::DeclarationId(declaration_id) => {
            let declaration = module.declarations.get(declaration_id);
            declaration_detail(db, ContainerId::ModuleId(module_id), declaration)
        }
        DeclaratorParent::StmtId(_) => None,
    }
}

fn non_ansi_port_detail(
    db: &dyn HirDb,
    module_id: ModuleId,
    module: &Module,
    entry: &NonAnsiPortEntry,
) -> Option<String> {
    if let Some(port_decl) = entry.port_decl
        && let Some(detail) = module_port_detail(db, module_id, module, port_decl)
    {
        return Some(detail);
    }

    entry.data_decl.and_then(|decl_id| module_decl_detail(db, module_id, module, decl_id))
}

fn declaration_detail(
    db: &dyn HirDb,
    container_id: ContainerId,
    declaration: &Declaration,
) -> Option<String> {
    use Declaration::*;
    match declaration {
        DataDecl(data_decl) => {
            let mut parts = Vec::new();
            if data_decl.const_kw {
                parts.push("const".to_string());
            }
            if data_decl.var_kw {
                parts.push("var".to_string());
            }
            if let Some(sig) = data_ty_signature(db, container_id, data_decl.ty) {
                parts.push(sig);
            }
            if parts.is_empty() { None } else { Some(parts.join(" ")) }
        }
        NetDecl(net_decl) => {
            let mut parts = Vec::new();
            if let Some(kind) = net_decl.net_kind {
                parts.push(net_kind_str(kind).to_string());
            }
            if let Some(sig) = data_ty_signature(db, container_id, net_decl.ty) {
                parts.push(sig);
            }
            if parts.is_empty() { None } else { Some(parts.join(" ")) }
        }
        ParamDecl(param_decl) => data_ty_signature(db, container_id, param_decl.ty),
    }
}

fn block_decl_detail(
    db: &dyn HirDb,
    block_id: BlockId,
    block: &Block,
    decl_id: DeclId,
) -> Option<String> {
    let declarator = block.decls.get(decl_id);
    match declarator.parent {
        DeclaratorParent::DeclarationId(declaration_id) => {
            let declaration = block.declarations.get(declaration_id);
            declaration_detail(db, ContainerId::BlockId(block_id), declaration)
        }
        DeclaratorParent::StmtId(_) | DeclaratorParent::PortDeclId(_) => None,
    }
}

fn subroutine_decl_detail(
    db: &dyn HirDb,
    loc: InModule<SubroutineId>,
    subroutine: &Subroutine,
    decl_id: DeclId,
) -> Option<String> {
    let declarator = subroutine.decls.get(decl_id);
    match declarator.parent {
        DeclaratorParent::DeclarationId(declaration_id) => {
            let declaration = subroutine.declarations.get(declaration_id);
            declaration_detail(db, ContainerId::SubroutineId(loc), declaration)
        }
        DeclaratorParent::StmtId(_) | DeclaratorParent::PortDeclId(_) => None,
    }
}

fn file_subroutine_decl_detail(
    db: &dyn HirDb,
    loc: InFile<SubroutineId>,
    subroutine: &Subroutine,
    decl_id: DeclId,
) -> Option<String> {
    let declarator = subroutine.decls.get(decl_id);
    match declarator.parent {
        DeclaratorParent::DeclarationId(declaration_id) => {
            let declaration = subroutine.declarations.get(declaration_id);
            declaration_detail(db, ContainerId::FileSubroutineId(loc), declaration)
        }
        DeclaratorParent::StmtId(_) | DeclaratorParent::PortDeclId(_) => None,
    }
}

fn package_decl_detail(
    db: &dyn HirDb,
    package_id: PackageId,
    package: &Package,
    decl_id: DeclId,
) -> Option<String> {
    let declarator = package.decls.get(decl_id);
    match declarator.parent {
        DeclaratorParent::DeclarationId(declaration_id) => {
            let declaration = package.declarations.get(declaration_id);
            declaration_detail(db, ContainerId::PackageId(package_id), declaration)
        }
        DeclaratorParent::StmtId(_) | DeclaratorParent::PortDeclId(_) => None,
    }
}

fn file_decl_detail(
    db: &dyn HirDb,
    file_id: HirFileId,
    file: &crate::hir_def::file::HirFile,
    decl_id: DeclId,
) -> Option<String> {
    let declarator = file.decls.get(decl_id);
    match declarator.parent {
        DeclaratorParent::DeclarationId(declaration_id) => {
            let declaration = file.declarations.get(declaration_id);
            declaration_detail(db, ContainerId::HirFileId(file_id), declaration)
        }
        DeclaratorParent::StmtId(_) | DeclaratorParent::PortDeclId(_) => None,
    }
}

impl PackageScope {
    pub fn package_scope_query(db: &dyn HirDb, package_id: PackageId) -> Arc<PackageScope> {
        let mut scope = Scope::default();
        let package = db.package(package_id);

        for (decl_id, decl) in package.decls.iter() {
            scope.insert_opt(&decl.name, InPackage::new(package_id, decl_id).into());
        }

        for (typedef_id, typedef_) in package.typedefs.iter() {
            scope.insert_opt(&typedef_.name, InPackage::new(package_id, typedef_id).into());
        }

        for (class_id, class_) in package.classes.iter() {
            scope.insert_opt(&class_.name, InPackage::new(package_id, class_id).into());
        }

        for (struct_id, struct_) in package.structs.iter() {
            scope.insert_opt(&struct_.name, InPackage::new(package_id, struct_id).into());
        }

        for (sub_id, subroutine) in package.subroutines.iter() {
            scope.insert_opt(&subroutine.name, InPackage::new(package_id, sub_id).into());
        }

        let file = db.hir_file(package_id.file_id);
        for (local_pkg_id, pkg_info) in file.packages.iter() {
            if pkg_info.parent == Some(package_id.value) {
                scope.insert_opt(
                    &pkg_info.name,
                    InPackage::new(package_id, InFile::new(package_id.file_id, local_pkg_id))
                        .into(),
                );
            }
        }

        let packages_by_name = db.packages_by_name();
        let mut package_cache: FxHashMap<PackageId, Arc<Package>> = FxHashMap::default();

        let mut get_package = |pkg_id: PackageId| {
            package_cache.entry(pkg_id).or_insert_with(|| db.package(pkg_id)).clone()
        };

        for (_, export) in package.exports.iter() {
            match export {
                PackageExport::All => {
                    // Re-exporting the current package does not introduce new
                    // entries.
                }
                PackageExport::Items(items) => {
                    for item in items {
                        let Some(target_packages) = packages_by_name.get(&item.package) else {
                            continue;
                        };

                        match &item.member {
                            PackageImportMember::All => {
                                for target_pkg_id in target_packages {
                                    let target_package = get_package(*target_pkg_id);

                                    for (decl_id, decl) in target_package.decls.iter() {
                                        if let Some(name) = &decl.name
                                            && scope.get(name).is_none()
                                        {
                                            scope.insert(
                                                name,
                                                InPackage::new(*target_pkg_id, decl_id).into(),
                                            );
                                        }
                                    }

                                    for (typedef_id, typedef_) in target_package.typedefs.iter() {
                                        if let Some(name) = &typedef_.name
                                            && scope.get(name).is_none()
                                        {
                                            scope.insert(
                                                name,
                                                InPackage::new(*target_pkg_id, typedef_id).into(),
                                            );
                                        }
                                    }

                                    for (class_id, class_) in target_package.classes.iter() {
                                        if let Some(name) = &class_.name
                                            && scope.get(name).is_none()
                                        {
                                            scope.insert(
                                                name,
                                                InPackage::new(*target_pkg_id, class_id).into(),
                                            );
                                        }
                                    }

                                    for (struct_id, struct_) in target_package.structs.iter() {
                                        if let Some(name) = &struct_.name
                                            && scope.get(name).is_none()
                                        {
                                            scope.insert(
                                                name,
                                                InPackage::new(*target_pkg_id, struct_id).into(),
                                            );
                                        }
                                    }

                                    for (sub_id, subroutine) in target_package.subroutines.iter() {
                                        if let Some(name) = &subroutine.name
                                            && scope.get(name).is_none()
                                        {
                                            scope.insert(
                                                name,
                                                InPackage::new(*target_pkg_id, sub_id).into(),
                                            );
                                        }
                                    }
                                }
                            }
                            PackageImportMember::Named(name) => {
                                if scope.get(name).is_some() {
                                    continue;
                                }

                                let mut found_entry = None;

                                for target_pkg_id in target_packages {
                                    let target_package = get_package(*target_pkg_id);

                                    found_entry = target_package
                                        .decls
                                        .iter()
                                        .find_map(|(decl_id, decl)| {
                                            (decl.name.as_ref() == Some(name)).then_some(
                                                InPackage::new(*target_pkg_id, decl_id).into(),
                                            )
                                        })
                                        .or_else(|| {
                                            target_package.typedefs.iter().find_map(
                                                |(typedef_id, ty)| {
                                                    ty.name
                                                        .as_ref()
                                                        .filter(|ident| *ident == name)
                                                        .map(|_| {
                                                            InPackage::new(
                                                                *target_pkg_id,
                                                                typedef_id,
                                                            )
                                                            .into()
                                                        })
                                                },
                                            )
                                        })
                                        .or_else(|| {
                                            target_package.classes.iter().find_map(
                                                |(class_id, class_)| {
                                                    class_
                                                        .name
                                                        .as_ref()
                                                        .filter(|ident| *ident == name)
                                                        .map(|_| {
                                                            InPackage::new(*target_pkg_id, class_id)
                                                                .into()
                                                        })
                                                },
                                            )
                                        })
                                        .or_else(|| {
                                            target_package.structs.iter().find_map(
                                                |(struct_id, struct_)| {
                                                    struct_
                                                        .name
                                                        .as_ref()
                                                        .filter(|ident| *ident == name)
                                                        .map(|_| {
                                                            InPackage::new(
                                                                *target_pkg_id,
                                                                struct_id,
                                                            )
                                                            .into()
                                                        })
                                                },
                                            )
                                        })
                                        .or_else(|| {
                                            target_package.subroutines.iter().find_map(
                                                |(sub_id, subroutine)| {
                                                    subroutine
                                                        .name
                                                        .as_ref()
                                                        .filter(|ident| *ident == name)
                                                        .map(|_| {
                                                            InPackage::new(*target_pkg_id, sub_id)
                                                                .into()
                                                        })
                                                },
                                            )
                                        });

                                    if found_entry.is_some() {
                                        break;
                                    }
                                }

                                if let Some(entry) = found_entry {
                                    scope.insert(name, entry);
                                }
                            }
                        }
                    }
                }
            }
        }

        Arc::new(scope)
    }

    pub fn collect_completions(
        &self,
        db: &dyn HirDb,
        package_id: PackageId,
    ) -> Vec<CompletionEntry> {
        let mut items = Vec::new();
        let mut package_cache: FxHashMap<PackageId, Arc<Package>> = FxHashMap::default();

        let mut get_package = |pkg_id: PackageId| {
            package_cache.entry(pkg_id).or_insert_with(|| db.package(pkg_id)).clone()
        };

        // Ensure the current package is cached for direct lookups.
        let _ = get_package(package_id);

        for (ident, entry) in self.iter() {
            let completion = match entry {
                PackageEntry::DeclId(in_pkg_decl) => {
                    let pkg = get_package(in_pkg_decl.package_id);
                    let declarator = pkg.decls.get(in_pkg_decl.value);
                    let (kind, detail) = match declarator.parent {
                        DeclaratorParent::DeclarationId(declaration_id) => {
                            let declaration = pkg.declarations.get(declaration_id);
                            let kind = match declaration {
                                Declaration::ParamDecl(_) => CompletionEntryKind::Parameter,
                                Declaration::NetDecl(_) => CompletionEntryKind::Net,
                                Declaration::DataDecl(_) => CompletionEntryKind::Variable,
                            };
                            let detail = declaration_detail(
                                db,
                                ContainerId::PackageId(in_pkg_decl.package_id),
                                declaration,
                            );
                            (kind, detail)
                        }
                        DeclaratorParent::StmtId(_) | DeclaratorParent::PortDeclId(_) => {
                            (CompletionEntryKind::Variable, None)
                        }
                    };
                    Some(make_entry_with_detail(ident, kind, detail))
                }
                PackageEntry::TypedefId(in_pkg_typedef) => {
                    let pkg = get_package(in_pkg_typedef.package_id);
                    let typedef = pkg.typedefs.get(in_pkg_typedef.value);
                    let detail = typedef_detail(
                        db,
                        ContainerId::PackageId(in_pkg_typedef.package_id),
                        typedef,
                    );
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Type, detail))
                }
                PackageEntry::ClassId(_) => Some(make_entry(ident, CompletionEntryKind::Type)),
                PackageEntry::StructId(_) => Some(make_entry(ident, CompletionEntryKind::Type)),
                PackageEntry::ProcId(_) => Some(make_entry(ident, CompletionEntryKind::Function)),
                PackageEntry::SubroutineId(in_pkg_sub) => {
                    let pkg = get_package(in_pkg_sub.package_id);
                    let sub = pkg.subroutines.get(in_pkg_sub.value);
                    let detail = match sub.kind {
                        SubroutineKind::Task => "task".to_string(),
                        SubroutineKind::Function { .. } => "function".to_string(),
                    };
                    Some(make_entry_with_detail(ident, CompletionEntryKind::Function, Some(detail)))
                }
                PackageEntry::Package(_) => Some(make_entry_with_detail(
                    ident,
                    CompletionEntryKind::Module,
                    Some(String::from("package")),
                )),
            };

            if let Some(entry) = completion {
                items.push(entry);
            }
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use base_db::{
        salsa,
        source_db::{FileLoader, SourceDb, SourceRootDb},
        source_root::{SourceRoot, SourceRootId},
    };
    use rustc_hash::FxHashSet;
    use smol_str::SmolStr;
    use triomphe::Arc;
    use vfs::{FileId, FileSet, VfsPath, anchored_path::AnchoredPath};

    use super::{BlockEntry, ModuleEntry, ModuleScope, PackageEntry, UnitEntry};
    use crate::{
        CompletionEntryKind,
        container::{InFile, InPackage},
        db::HirDb,
        file::HirFileId,
        hir_def::module::ModuleId,
    };

    #[salsa::database(
        base_db::source_db::SourceDbStorage,
        base_db::source_db::SourceRootDbStorage,
        crate::db::InternDbStorage,
        crate::db::HirDbStorage
    )]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    impl Default for TestDb {
        fn default() -> Self {
            TestDb { storage: salsa::Storage::default() }
        }
    }

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl salsa::Database for TestDb {}

    impl FileLoader for TestDb {
        fn resolve_path(&self, _path: AnchoredPath<'_>) -> Option<FileId> {
            None
        }
    }

    fn setup_db(text: &str) -> (TestDb, HirFileId) {
        let mut db = TestDb::default();

        let file_id = FileId(0);

        let mut files = FxHashSet::default();
        files.insert(file_id);
        db.set_files(Box::new(files));

        db.set_file_text(file_id, Arc::from(text.to_string()));

        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/test.sv".into()));
        let source_root = SourceRoot::new_local(file_set);
        let source_root_id = SourceRootId(0);
        db.set_source_root_id(file_id, source_root_id);
        db.set_source_root(source_root_id, Arc::new(source_root));

        (db, HirFileId(file_id))
    }

    fn setup_multi_file_db(files: &[(&str, &str)]) -> (TestDb, Vec<HirFileId>) {
        let mut db = TestDb::default();

        let mut file_ids = Vec::with_capacity(files.len());
        let mut files_set = FxHashSet::default();

        for (idx, _) in files.iter().enumerate() {
            let file_id = FileId(idx as u32);
            file_ids.push(HirFileId(file_id));
            files_set.insert(file_id);
        }

        db.set_files(Box::new(files_set));

        let mut file_set = FileSet::default();
        let source_root_id = SourceRootId(0);

        for (idx, (path, text)) in files.iter().enumerate() {
            let file_id = FileId(idx as u32);
            db.set_file_text(file_id, Arc::from((*text).to_string()));
            db.set_source_root_id(file_id, source_root_id);
            file_set.insert(file_id, VfsPath::new_virtual_path((*path).into()));
        }

        let source_root = SourceRoot::new_local(file_set);
        db.set_source_root(source_root_id, Arc::new(source_root));

        (db, file_ids)
    }

    fn module_scope(db: &TestDb, file_id: HirFileId, name: &str) -> (ModuleId, Arc<ModuleScope>) {
        let scope = HirDb::file_scope(db, file_id);
        let entry = scope.get(&SmolStr::new(name)).expect("module is present in unit scope");
        let module_id = match entry {
            UnitEntry::ModuleId(module_id) => module_id,
            _ => panic!("expected module entry"),
        };
        (module_id, HirDb::module_scope(db, module_id))
    }

    #[test]
    fn unit_scope_contains_modules_and_file_declarations() {
        let text = r#"
module leaf();
endmodule

module top();
endmodule

wire global_wire;
"#;

        let (db, file_id) = setup_db(text);
        let scope = HirDb::file_scope(&db, file_id);

        let leaf_entry = scope.get(&SmolStr::new("leaf")).expect("leaf present");
        let leaf_module_id = match leaf_entry {
            UnitEntry::ModuleId(module_id) => module_id,
            _ => panic!("expected module"),
        };
        assert_eq!(leaf_module_id.file_id(), file_id.file_id());

        let global_entry = scope.get(&SmolStr::new("global_wire")).expect("global wire present");
        match global_entry {
            UnitEntry::FiledDeclId(InFile { file_id: decl_file, .. }) => {
                assert_eq!(decl_file, file_id);
            }
            _ => panic!("expected file-level declaration"),
        }

        let completions = scope.collect_completions(&db);
        let labels: Vec<_> = completions.iter().map(|entry| entry.name.clone()).collect();
        assert!(labels.contains(&SmolStr::new("leaf")));
        assert!(labels.contains(&SmolStr::new("top")));
        assert!(labels.contains(&SmolStr::new("global_wire")));
    }

    #[test]
    fn unit_scope_includes_top_level_classes() {
        let text = r#"
class pkt;
  int data;
endclass

module top;
endmodule
"#;

        let (db, file_id) = setup_db(text);
        let scope = HirDb::file_scope(&db, file_id);

        let class_entry = scope.get(&SmolStr::new("pkt")).expect("class present");
        assert!(matches!(class_entry, UnitEntry::ClassId(_)));

        let completions = scope.collect_completions(&db);
        let mut seen_class = false;
        for entry in completions {
            if entry.name.as_str() == "pkt" {
                assert_eq!(entry.kind, CompletionEntryKind::Type);
                seen_class = true;
            }
        }
        assert!(seen_class, "expected class completion");
    }

    #[test]
    fn module_scope_classifies_members() {
        let text = r#"
module leaf();
endmodule

module non_ansi(clk);
  input logic clk;
  logic clk;
endmodule

module top(
  input logic a,
  output logic b
);
  non_ansi u0(.clk(a));
  logic data;

  import pkg::foo;

  class my_class;
    int value;
  endclass

  initial begin : init_blk
    logic inner;
    nested_stmt: inner = 1'b0;
    begin : nested_blk
    end
  end
endmodule
"#;

        let (db, file_id) = setup_db(text);

        let (_, non_ansi_scope) = module_scope(&db, file_id, "non_ansi");

        let clk_entry = non_ansi_scope.get(&SmolStr::new("clk")).expect("clk in scope");
        match clk_entry {
            ModuleEntry::NonAnsiPortEntry(entry) => {
                assert!(entry.label.is_some(), "non-ANSI port keeps label");
                assert!(entry.port_decl.is_some(), "non-ANSI port tracks port decl");
                assert!(entry.data_decl.is_some(), "non-ANSI port tracks data decl");
            }
            _ => panic!("expected non-ANSI port entry"),
        }

        let (top_module_id, top_scope) = module_scope(&db, file_id, "top");

        for name in ["a", "b"] {
            let entry = top_scope.get(&SmolStr::new(name)).expect("ANSI port present");
            assert!(matches!(entry, ModuleEntry::AnsiPortEntry(_)), "expected ANSI port entry");
        }

        let data_entry = top_scope.get(&SmolStr::new("data")).expect("module decl");
        assert!(matches!(data_entry, ModuleEntry::DeclId(_)));

        let instance_entry = top_scope.get(&SmolStr::new("u0")).expect("instance present");
        assert!(matches!(instance_entry, ModuleEntry::InstanceId(_)));

        let init_entry = top_scope.get(&SmolStr::new("init_blk")).expect("block in module scope");
        let init_block_id = match init_entry {
            ModuleEntry::BlockId(block_id) => block_id,
            _ => panic!("expected block id"),
        };

        let class_entry = top_scope.get(&SmolStr::new("my_class")).expect("class present");
        assert!(matches!(class_entry, ModuleEntry::ClassId(_)));

        let import_entry = top_scope.get(&SmolStr::new("foo")).expect("import present");
        match import_entry {
            ModuleEntry::PackageImportEntry(entry) => {
                assert_eq!(entry.item_idx, 0);
            }
            _ => panic!("expected package import entry"),
        }

        assert!(
            top_scope.get(&SmolStr::new("nested_stmt")).is_none(),
            "statement labels stay within block scope"
        );

        let block_scope = HirDb::block_scope(&db, init_block_id);

        let inner_decl = block_scope.get(&SmolStr::new("inner")).expect("block declaration");
        assert!(matches!(inner_decl, BlockEntry::DeclId(_)));

        let nested_stmt = block_scope.get(&SmolStr::new("nested_stmt")).expect("stmt label");
        assert!(matches!(nested_stmt, BlockEntry::StmtId(_)));

        let nested_block = block_scope.get(&SmolStr::new("nested_blk")).expect("nested block");
        assert!(matches!(nested_block, BlockEntry::BlockId(_)));

        let module_items = top_scope.collect_completions(&db, top_module_id);
        let labels: Vec<_> = module_items.iter().map(|entry| entry.name.clone()).collect();
        assert!(labels.contains(&SmolStr::new("data")));
        assert!(labels.contains(&SmolStr::new("u0")));
        assert!(labels.contains(&SmolStr::new("init_blk")));
        assert!(labels.contains(&SmolStr::new("my_class")));
        assert!(labels.contains(&SmolStr::new("foo")));

        let import_completion = module_items
            .iter()
            .find(|entry| entry.name == SmolStr::new("foo"))
            .expect("import completion");
        assert_eq!(import_completion.kind, CompletionEntryKind::Import);

        let block_items = block_scope.collect_completions(&db, init_block_id);
        let block_labels: Vec<_> = block_items.iter().map(|entry| entry.name.clone()).collect();
        assert!(block_labels.contains(&SmolStr::new("inner")));
        assert!(block_labels.contains(&SmolStr::new("nested_stmt")));
        assert!(block_labels.contains(&SmolStr::new("nested_blk")));
    }

    #[test]
    fn package_scope_collects_members() {
        let text = r#"
package my_pkg;
  typedef struct {
    int value;
  } pkt_t;

  int data;

  class my_class;
    int field;
  endclass

  function automatic int compute();
  endfunction
endpackage
"#;

        let (db, file_id) = setup_db(text);
        let unit_scope = HirDb::file_scope(&db, file_id);
        let pkg_entry = unit_scope.get(&SmolStr::new("my_pkg")).expect("package present");
        let package_id = match pkg_entry {
            UnitEntry::PackageId(package_id) => package_id,
            _ => panic!("expected package entry"),
        };

        let package_scope = HirDb::package_scope(&db, package_id);

        let data_entry = package_scope.get(&SmolStr::new("data")).expect("data declaration");
        match data_entry {
            PackageEntry::DeclId(InPackage { package_id: owner, .. }) => {
                assert_eq!(owner, package_id);
            }
            _ => panic!("expected decl entry"),
        }

        let typedef_entry = package_scope.get(&SmolStr::new("pkt_t")).expect("typedef entry");
        match typedef_entry {
            PackageEntry::TypedefId(InPackage { package_id: owner, .. }) => {
                assert_eq!(owner, package_id);
            }
            _ => panic!("expected typedef entry"),
        }

        let class_entry = package_scope.get(&SmolStr::new("my_class")).expect("class entry");
        match class_entry {
            PackageEntry::ClassId(InPackage { package_id: owner, .. }) => {
                assert_eq!(owner, package_id);
            }
            _ => panic!("expected class entry"),
        }

        let func_entry = package_scope.get(&SmolStr::new("compute")).expect("function entry");
        match func_entry {
            PackageEntry::SubroutineId(InPackage { package_id: owner, .. }) => {
                assert_eq!(owner, package_id);
            }
            _ => panic!("expected subroutine entry"),
        }

        let completions = package_scope.collect_completions(&db, package_id);
        let mut seen =
            completions.into_iter().map(|entry| (entry.name, entry.kind)).collect::<Vec<_>>();
        seen.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            seen,
            vec![
                (SmolStr::new("compute"), CompletionEntryKind::Function),
                (SmolStr::new("data"), CompletionEntryKind::Variable),
                (SmolStr::new("my_class"), CompletionEntryKind::Type),
                (SmolStr::new("pkt_t"), CompletionEntryKind::Type),
            ]
        );
    }

    #[test]
    fn package_scope_includes_package_exports() {
        let text = r#"
package inner_pkg;
  typedef int inner_t;
  int inner_var;
  function automatic void inner_fn();
  endfunction
endpackage

package outer_pkg;
  import inner_pkg::*;
  export inner_pkg::inner_var;
  export inner_pkg::*;
endpackage
"#;

        let (db, file_id) = setup_db(text);
        let unit_scope = HirDb::file_scope(&db, file_id);

        let inner_pkg_entry = unit_scope.get(&SmolStr::new("inner_pkg")).expect("inner package");
        let inner_pkg_id = match inner_pkg_entry {
            UnitEntry::PackageId(package_id) => package_id,
            _ => panic!("expected package entry"),
        };

        let outer_pkg_entry = unit_scope.get(&SmolStr::new("outer_pkg")).expect("outer package");
        let outer_pkg_id = match outer_pkg_entry {
            UnitEntry::PackageId(package_id) => package_id,
            _ => panic!("expected package entry"),
        };

        let outer_scope = HirDb::package_scope(&db, outer_pkg_id);

        let inner_var_entry =
            outer_scope.get(&SmolStr::new("inner_var")).expect("inner var re-exported");
        match inner_var_entry {
            PackageEntry::DeclId(InPackage { package_id, .. }) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected declaration entry"),
        }

        let inner_t_entry = outer_scope.get(&SmolStr::new("inner_t")).expect("typedef export");
        match inner_t_entry {
            PackageEntry::TypedefId(InPackage { package_id, .. }) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected typedef entry"),
        }

        let inner_fn_entry = outer_scope.get(&SmolStr::new("inner_fn")).expect("function export");
        match inner_fn_entry {
            PackageEntry::SubroutineId(InPackage { package_id, .. }) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected subroutine entry"),
        }

        let completions = outer_scope.collect_completions(&db, outer_pkg_id);
        let mut seen =
            completions.into_iter().map(|entry| (entry.name, entry.kind)).collect::<Vec<_>>();
        seen.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            seen,
            vec![
                (SmolStr::new("inner_fn"), CompletionEntryKind::Function),
                (SmolStr::new("inner_t"), CompletionEntryKind::Type),
                (SmolStr::new("inner_var"), CompletionEntryKind::Variable),
            ]
        );
    }

    #[test]
    fn module_scope_resolves_package_imports() {
        let text = r#"
package inner_pkg;
  typedef int inner_t;
  int inner_var;
  function automatic void inner_fn();
  endfunction
endpackage

module top;
  import inner_pkg::*;
endmodule
"#;

        let (db, file_id) = setup_db(text);
        let unit_scope = HirDb::file_scope(&db, file_id);

        let inner_pkg_entry = unit_scope.get(&SmolStr::new("inner_pkg")).expect("inner package");
        let inner_pkg_id = match inner_pkg_entry {
            UnitEntry::PackageId(package_id) => package_id,
            _ => panic!("expected package entry"),
        };

        let (module_id, module_scope) = module_scope(&db, file_id, "top");

        let import_typedef =
            module_scope.get(&SmolStr::new("inner_t")).expect("typedef import present");
        match import_typedef {
            ModuleEntry::PackageMember(PackageEntry::TypedefId(InPackage {
                package_id, ..
            })) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected typedef import entry"),
        }

        let import_var =
            module_scope.get(&SmolStr::new("inner_var")).expect("variable import present");
        match import_var {
            ModuleEntry::PackageMember(PackageEntry::DeclId(InPackage { package_id, .. })) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected variable import entry"),
        }

        let import_fn =
            module_scope.get(&SmolStr::new("inner_fn")).expect("function import present");
        match import_fn {
            ModuleEntry::PackageMember(PackageEntry::SubroutineId(InPackage {
                package_id,
                ..
            })) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected subroutine import entry"),
        }

        let completions = module_scope.collect_completions(&db, module_id);
        let mut labels = completions.iter().map(|entry| entry.name.clone()).collect::<Vec<_>>();
        labels.sort();
        assert!(labels.contains(&SmolStr::new("inner_t")));
        assert!(labels.contains(&SmolStr::new("inner_var")));
        assert!(labels.contains(&SmolStr::new("inner_fn")));
    }

    #[test]
    fn package_exports_across_files() {
        let files = [
            (
                "/inner.sv",
                r#"
package inner_pkg;
  typedef int inner_t;
  int inner_var;
  function automatic int inner_fn();
  endfunction
endpackage
"#,
            ),
            (
                "/outer.sv",
                r#"
package outer_pkg;
  import inner_pkg::*;
  export inner_pkg::inner_var;
  export inner_pkg::*;
endpackage
"#,
            ),
            (
                "/consumer.sv",
                r#"
module consumer;
  import outer_pkg::*;
endmodule
"#,
            ),
        ];

        let (db, file_ids) = setup_multi_file_db(&files);
        let unit_scope = HirDb::unit_scope(&db);

        let inner_pkg_entry = unit_scope.get(&SmolStr::new("inner_pkg")).expect("inner pkg");
        let inner_pkg_id = match inner_pkg_entry {
            UnitEntry::PackageId(package_id) => package_id,
            _ => panic!("expected package entry"),
        };

        let outer_pkg_entry = unit_scope.get(&SmolStr::new("outer_pkg")).expect("outer pkg");
        let outer_pkg_id = match outer_pkg_entry {
            UnitEntry::PackageId(package_id) => package_id,
            _ => panic!("expected package entry"),
        };

        let outer_scope = HirDb::package_scope(&db, outer_pkg_id);

        let reexported_var =
            outer_scope.get(&SmolStr::new("inner_var")).expect("re-exported variable present");
        match reexported_var {
            PackageEntry::DeclId(InPackage { package_id, .. }) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected declaration entry"),
        }

        let reexported_typedef =
            outer_scope.get(&SmolStr::new("inner_t")).expect("re-exported typedef present");
        match reexported_typedef {
            PackageEntry::TypedefId(InPackage { package_id, .. }) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected typedef entry"),
        }

        let reexported_fn =
            outer_scope.get(&SmolStr::new("inner_fn")).expect("re-exported function present");
        match reexported_fn {
            PackageEntry::SubroutineId(InPackage { package_id, .. }) => {
                assert_eq!(package_id, inner_pkg_id);
            }
            _ => panic!("expected subroutine entry"),
        }

        let (_module_id, consumer_scope) = module_scope(&db, file_ids[2], "consumer");

        let imported_var = consumer_scope
            .get(&SmolStr::new("inner_var"))
            .expect("module sees re-exported variable");
        match imported_var {
            ModuleEntry::PackageMember(entry) => match entry {
                PackageEntry::DeclId(InPackage { package_id, .. }) => {
                    assert_eq!(package_id, inner_pkg_id);
                }
                _ => panic!("expected declaration package member"),
            },
            _ => panic!("expected package member"),
        }

        let imported_type =
            consumer_scope.get(&SmolStr::new("inner_t")).expect("module sees re-exported typedef");
        match imported_type {
            ModuleEntry::PackageMember(entry) => match entry {
                PackageEntry::TypedefId(InPackage { package_id, .. }) => {
                    assert_eq!(package_id, inner_pkg_id);
                }
                _ => panic!("expected typedef package member"),
            },
            _ => panic!("expected package member"),
        }

        let imported_fn = consumer_scope
            .get(&SmolStr::new("inner_fn"))
            .expect("module sees re-exported function");
        match imported_fn {
            ModuleEntry::PackageMember(entry) => match entry {
                PackageEntry::SubroutineId(InPackage { package_id, .. }) => {
                    assert_eq!(package_id, inner_pkg_id);
                }
                _ => panic!("expected subroutine package member"),
            },
            _ => panic!("expected package member"),
        }
    }
}
