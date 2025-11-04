use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::define_enum_deriving_from;

use crate::{
    container::{InFile, InModule, InPackage},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        Ident,
        aggregate::{ClassId, StructId},
        block::{BlockId, BlockInfo},
        expr::declarator::{DeclId, DeclaratorParent},
        module::{
            ModuleId,
            instantiation::InstanceId,
            port::{NonAnsiPortId, Ports},
        },
        package::{Package, PackageExport, PackageId, PackageImportId, PackageImportMember},
        proc::ProcId,
        stmt::{StmtId, StmtKind},
        subroutine::SubroutineId,
        typedef::TypedefId,
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
        SubroutineId(InFile<SubroutineId>),
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

        for (subroutine_id, subroutine_info) in hir_file.subroutines.iter() {
            scope.insert_opt(&subroutine_info.name, InFile::new(file_id, subroutine_id).into());
        }

        Arc::new(scope)
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
}
