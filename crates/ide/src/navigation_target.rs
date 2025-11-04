use base_db::intern::Lookup;
use hir::{
    container::{ContainerId, InContainer, InFile, InModule},
    db::HirDb,
    hir_def::{
        DEFAULT_NAME,
        aggregate::ClassId,
        block::{BlockId, BlockLoc},
        declaration::Declaration,
        expr::declarator::{DeclId, DeclaratorParent},
        module::{ModuleId, instantiation::InstanceId, port::NonAnsiPortId},
        package::{PackageId, PackageImportMember},
        stmt::StmtId,
        subroutine::SubroutineId,
        typedef::TypedefId,
    },
    scope::PackageImportEntry,
    source_map::{IsNamedSrc, IsSrc},
};
use ide_db::root_db::RootDb;
use smol_str::SmolStr;
use syntax::{SyntaxTokenWithParent, has_text_range::HasTextRange};
use utils::{
    get::{Get, GetRef},
    line_index::{TextRange, TextSize},
};
use vfs::FileId;

use crate::{SymbolKind, definitions::DefinitionOrigin};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NavTarget {
    pub file_id: FileId,
    pub full_range: TextRange,
    pub focus_range: Option<TextRange>,

    pub name: Option<SmolStr>,
    pub kind: Option<SymbolKind>,
    pub container_name: Option<SmolStr>,
    // TODO: how to represent this?
    pub description: Option<String>,
}

impl NavTarget {
    pub fn focus_or_full_range(&self) -> TextRange {
        self.focus_range.unwrap_or(self.full_range)
    }
}

pub(crate) trait ToNav {
    fn to_nav(&self, db: &RootDb) -> NavTarget;
}

impl ToNav for DefinitionOrigin {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        match self {
            DefinitionOrigin::ModuleId(module_id) => module_id.to_nav(db),
            DefinitionOrigin::BlockId(block_id) => block_id.to_nav(db),
            DefinitionOrigin::NonAnsiPort(nonansi_port_id) => nonansi_port_id.to_nav(db),
            DefinitionOrigin::Decl(decl_id) => decl_id.to_nav(db),
            DefinitionOrigin::Instance(instance_id) => instance_id.to_nav(db),
            DefinitionOrigin::Stmt(stmt_id) => stmt_id.to_nav(db),
            DefinitionOrigin::Typedef(typedef_id) => typedef_id.to_nav(db),
            DefinitionOrigin::Class(class_id) => class_id.to_nav(db),
            DefinitionOrigin::Package(package_id) => package_id.to_nav(db),
            DefinitionOrigin::PackageImport(import) => import.to_nav(db),
            DefinitionOrigin::Subroutine(sub) => sub.to_nav(db),
        }
    }
}

impl ToNav for ModuleId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InFile { value: local_module_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(local_module_id);
        let name = self.to_container(db).name.clone();

        let file_id = file_id.file_id();
        build(file_id, src.name_range(), src.range(), name, SymbolKind::Module, None)
    }
}

impl ToNav for BlockId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let BlockLoc { cont_id, src: InFile { value: src, file_id } } = self.lookup(db);
        let name = self.to_container(db).name.clone();
        let cont_name = cont_id.to_container(db).name().cloned();

        let file_id = file_id.file_id();
        build(file_id, src.name_range(), src.range(), name, SymbolKind::Block, cont_name)
    }
}

impl ToNav for InModule<NonAnsiPortId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InModule { value: port_id, module_id } = *self;

        let file_id = module_id.file_id;
        let src = module_id.to_container_src_map(db).get(port_id);

        let module = db.module(module_id);
        let name = module.get(port_id).label.clone();
        let cont_name = module.name.clone();

        let file_id = file_id.file_id();
        build(file_id, src.name_range(), src.range(), name, SymbolKind::NonAnsiPortLabel, cont_name)
    }
}

impl ToNav for InContainer<DeclId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: decl_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.to_container_src_map(db).get(decl_id);

        let cont = cont_id.to_container(db);
        let decl = cont.get(decl_id);

        let kind = match decl.parent {
            DeclaratorParent::PortDeclId(_) => SymbolKind::PortDecl,
            DeclaratorParent::DeclarationId(idx) => match cont.get(idx) {
                Declaration::DataDecl(_) => SymbolKind::DataDecl,
                Declaration::NetDecl(_) => SymbolKind::NetDecl,
                Declaration::ParamDecl(_) => SymbolKind::ParamDecl,
            },
            DeclaratorParent::StmtId(_) => SymbolKind::DataDecl,
        };

        let name = decl.name.clone();
        let cont_name = cont.name().cloned();

        build(file_id, src.name_range(), src.range(), name, kind, cont_name)
    }
}

impl ToNav for InModule<InstanceId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InModule { value: instance_id, module_id } = *self;

        let file_id = module_id.file_id();
        let src = module_id.to_container_src_map(db).get(instance_id);

        let module = module_id.to_container(db);
        let name = module.get(instance_id).name.clone();
        let cont_name = module.name.clone();

        build(file_id, src.name_range(), src.range(), name, SymbolKind::Instance, cont_name)
    }
}

impl ToNav for InContainer<StmtId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: stmt_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.to_container_src_map(db).get(stmt_id);

        let cont = cont_id.to_container(db);
        let name = cont.get(stmt_id).label.clone();
        let cont_name = cont.name().cloned();

        build(file_id, src.name_range(), src.range(), name, SymbolKind::Stmt, cont_name)
    }
}

impl ToNav for InContainer<TypedefId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: typedef_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.to_container_src_map(db).get(typedef_id);

        let cont = cont_id.to_container(db);
        let typedef = cont.get(typedef_id);
        let name = typedef.name.clone();
        let cont_name = cont.name().cloned();

        build(file_id, src.name_range(), src.range(), name, SymbolKind::Typedef, cont_name)
    }
}

impl ToNav for InContainer<ClassId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: class_id, cont_id } = *self;

        match cont_id {
            ContainerId::HirFileId(file_id) => {
                let src = file_id.to_container_src_map(db).get(class_id);
                let file = file_id.to_container(db);
                let name = file.classes.get(class_id).name.clone();

                let file_id = file_id.file_id();
                build(file_id, src.name_range(), src.range(), name, SymbolKind::Class, None)
            }
            ContainerId::ModuleId(module_id) => {
                let src = module_id.to_container_src_map(db).get(class_id);
                let module = module_id.to_container(db);
                let name = module.classes.get(class_id).name.clone();
                let cont_name = module.name.clone();

                let file_id = module_id.file_id.0;
                build(file_id, src.name_range(), src.range(), name, SymbolKind::Class, cont_name)
            }
            ContainerId::PackageId(package_id) => {
                let src = package_id.to_container_src_map(db).get(class_id);
                let package = package_id.to_container(db);
                let name = package.classes.get(class_id).name.clone();
                let cont_name = package.name.clone();

                let file_id = package_id.file_id.0;
                build(file_id, src.name_range(), src.range(), name, SymbolKind::Class, cont_name)
            }
            ContainerId::BlockId(block_id) => {
                let BlockLoc { cont_id: parent_cont, src: InFile { value: block_src, file_id } } =
                    block_id.lookup(db);
                let container_name = parent_cont.to_container(db).name().cloned();
                let focus = block_src.name_range();
                let full = block_src.range();
                build(
                    file_id.file_id(),
                    focus,
                    full,
                    Some(DEFAULT_NAME.clone()),
                    SymbolKind::Class,
                    container_name,
                )
            }
            ContainerId::SubroutineId(loc) => {
                let module_id = loc.module_id;
                let module = module_id.to_container(db);
                let cont_name = module.name.clone();
                let src = module_id.to_container_src_map(db).get(loc.value);
                build(
                    module_id.file_id.file_id(),
                    None,
                    src.range(),
                    Some(DEFAULT_NAME.clone()),
                    SymbolKind::Class,
                    cont_name,
                )
            }
            ContainerId::FileSubroutineId(InFile { file_id, value: sub_id }) => {
                let file = file_id.to_container(db);
                let subroutine = file.subroutines.get(sub_id);
                let cont_name = subroutine.name.clone();
                let src = file_id.to_container_src_map(db).get(sub_id);
                build(
                    file_id.file_id(),
                    None,
                    src.range(),
                    Some(DEFAULT_NAME.clone()),
                    SymbolKind::Class,
                    cont_name,
                )
            }
        }
    }
}

impl ToNav for InModule<PackageImportEntry> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InModule { value: entry, module_id } = *self;
        let module = module_id.to_container(db);
        let src = module_id.to_container_src_map(db).get(entry.import);
        let import = module.package_imports.get(entry.import);
        let label = render_package_import(import);

        let file_id = module_id.file_id.0;
        let cont_name = module.name.clone();
        build(file_id, None, src.range(), Some(label), SymbolKind::Import, cont_name)
    }
}

impl ToNav for PackageId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InFile { value: local_package_id, file_id } = *self;
        let file = file_id.to_container(db);
        let src_map = file_id.to_container_src_map(db);
        let src = src_map.get(local_package_id);
        let package = file.packages.get(local_package_id);
        let name = package.name.clone();

        build(file_id.file_id(), src.name_range(), src.range(), name, SymbolKind::Module, None)
    }
}

impl ToNav for InContainer<SubroutineId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: sub_id, cont_id } = *self;

        match cont_id {
            ContainerId::ModuleId(module_id) => {
                let module = module_id.to_container(db);
                let src = module_id.to_container_src_map(db).get(sub_id);
                let name = module
                    .subroutines
                    .get(sub_id)
                    .name
                    .clone()
                    .unwrap_or_else(|| DEFAULT_NAME.clone());
                let cont_name = module.name.clone();
                build(
                    module_id.file_id.file_id(),
                    None,
                    src.range(),
                    Some(name),
                    SymbolKind::Fn,
                    cont_name,
                )
            }
            ContainerId::PackageId(package_id) => {
                let pkg = package_id.to_container(db);
                let src = package_id.to_container_src_map(db).get(sub_id);
                let name = pkg
                    .subroutines
                    .get(sub_id)
                    .name
                    .clone()
                    .unwrap_or_else(|| DEFAULT_NAME.clone());
                let cont_name = pkg.name.clone();
                build(
                    package_id.file_id.file_id(),
                    None,
                    src.range(),
                    Some(name),
                    SymbolKind::Fn,
                    cont_name,
                )
            }
            ContainerId::HirFileId(file_id) => build(
                file_id.file_id(),
                None,
                TextRange::empty(TextSize::from(0)),
                Some(DEFAULT_NAME.clone()),
                SymbolKind::Fn,
                None,
            ),
            ContainerId::BlockId(block_id) => {
                let BlockLoc { cont_id: parent_cont, src: InFile { value: block_src, file_id } } =
                    block_id.lookup(db);
                let container_name = parent_cont.to_container(db).name().cloned();
                build(
                    file_id.file_id(),
                    block_src.name_range(),
                    block_src.range(),
                    Some(DEFAULT_NAME.clone()),
                    SymbolKind::Fn,
                    container_name,
                )
            }
            ContainerId::SubroutineId(loc) => {
                let module_id = loc.module_id;
                let module = module_id.to_container(db);
                let src = module_id.to_container_src_map(db).get(loc.value);
                let name = module
                    .subroutines
                    .get(loc.value)
                    .name
                    .clone()
                    .unwrap_or_else(|| DEFAULT_NAME.clone());
                let cont_name = module.name.clone();
                build(
                    module_id.file_id.file_id(),
                    None,
                    src.range(),
                    Some(name),
                    SymbolKind::Fn,
                    cont_name,
                )
            }
            ContainerId::FileSubroutineId(InFile { file_id, value: file_sub_id }) => {
                let file = file_id.to_container(db);
                let src = file_id.to_container_src_map(db).get(file_sub_id);
                let name = file
                    .subroutines
                    .get(file_sub_id)
                    .name
                    .clone()
                    .unwrap_or_else(|| DEFAULT_NAME.clone());
                build(file_id.file_id(), None, src.range(), Some(name), SymbolKind::Fn, None)
            }
        }
    }
}

impl ToNav for InFile<SyntaxTokenWithParent<'_>> {
    fn to_nav(&self, _db: &RootDb) -> NavTarget {
        let InFile { value: SyntaxTokenWithParent { parent, tok }, file_id } = *self;
        NavTarget {
            file_id: file_id.file_id(),
            full_range: parent.text_range().unwrap(),
            focus_range: tok.text_range(),
            name: None,
            kind: None,
            container_name: None,
            description: None,
        }
    }
}

#[inline]
fn build(
    file_id: FileId,
    focus_range: Option<TextRange>,
    full_range: TextRange,
    name: Option<SmolStr>,
    kind: SymbolKind,
    container_name: Option<SmolStr>,
) -> NavTarget {
    let kind = Some(kind);
    NavTarget { file_id, full_range, focus_range, name, kind, container_name, description: None }
}

fn render_package_import(import: &hir::hir_def::package::PackageImport) -> SmolStr {
    let mut parts = Vec::new();
    for item in import.items.iter() {
        match &item.member {
            PackageImportMember::Named(name) => {
                parts.push(format!("{}::{}", item.package, name));
            }
            PackageImportMember::All => {
                parts.push(format!("{}::*", item.package));
            }
        }
    }
    SmolStr::from(format!("import {}", parts.join(", ")))
}
