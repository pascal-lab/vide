use rustc_hash::FxHashMap;
use triomphe::Arc;
use utils::get::GetRef;

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
            port::{PortDirection, PortHeader},
        },
        package::{Package, PackageId, PackageImportMember},
        proc::ProcId,
        stmt::{StmtId, StmtKind},
        subroutine::{Subroutine, SubroutineId, SubroutineKind},
        ty::NetKind,
        typedef::{Typedef, TypedefId},
    },
    scope::{
        AnsiPortEntry, BlockEntry, ModuleEntry, NonAnsiPortEntry, PackageEntry, PackageImportEntry,
        Scope as RawScope, SubroutineEntry, UnitEntry,
    },
};

pub trait UnitScopeCompletionExt {
    fn collect_completions(&self, db: &dyn HirDb) -> Vec<CompletionEntry>;
}

impl UnitScopeCompletionExt for RawScope<UnitEntry> {
    fn collect_completions(&self, db: &dyn HirDb) -> Vec<CompletionEntry> {
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
                UnitEntry::SubroutineId(in_file) => {
                    let file = db.hir_file(in_file.file_id);
                    let subroutine = file.subroutines.get(in_file.value);
                    let detail = match subroutine.kind {
                        SubroutineKind::Task => "task".to_string(),
                        SubroutineKind::Function { .. } => "function".to_string(),
                    };
                    items.push(make_entry_with_detail(
                        ident,
                        CompletionEntryKind::Function,
                        Some(detail),
                    ));
                }
            }
        }

        items
    }
}

pub trait ModuleScopeCompletionExt {
    fn collect_completions(&self, db: &dyn HirDb, module_id: ModuleId) -> Vec<CompletionEntry>;
}

impl ModuleScopeCompletionExt for RawScope<ModuleEntry> {
    fn collect_completions(&self, db: &dyn HirDb, module_id: ModuleId) -> Vec<CompletionEntry> {
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

pub trait BlockScopeCompletionExt {
    fn collect_completions(&self, db: &dyn HirDb, block_id: BlockId) -> Vec<CompletionEntry>;
}

impl BlockScopeCompletionExt for RawScope<BlockEntry> {
    fn collect_completions(&self, db: &dyn HirDb, block_id: BlockId) -> Vec<CompletionEntry> {
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

pub trait SubroutineScopeCompletionExt {
    fn collect_completions(
        &self,
        db: &dyn HirDb,
        subroutine_loc: InModule<SubroutineId>,
    ) -> Vec<CompletionEntry>;

    fn collect_file_subroutine_completions(
        &self,
        db: &dyn HirDb,
        subroutine_loc: InFile<SubroutineId>,
    ) -> Vec<CompletionEntry>;
}

impl SubroutineScopeCompletionExt for RawScope<SubroutineEntry> {
    fn collect_completions(
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

    fn collect_file_subroutine_completions(
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
                    let detail =
                        file_subroutine_decl_detail(db, subroutine_loc, &subroutine, *decl_id);
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

pub trait PackageScopeCompletionExt {
    fn collect_completions(&self, db: &dyn HirDb, package_id: PackageId) -> Vec<CompletionEntry>;
}

impl PackageScopeCompletionExt for RawScope<PackageEntry> {
    fn collect_completions(&self, db: &dyn HirDb, package_id: PackageId) -> Vec<CompletionEntry> {
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
