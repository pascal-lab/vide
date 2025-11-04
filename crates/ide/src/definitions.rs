use base_db::intern::Lookup;
use hir::{
    container::{ContainerId, InContainer, InFile, InModule},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        DEFAULT_NAME,
        aggregate::ClassId,
        block::{BlockId, BlockLoc},
        expr::declarator::DeclId,
        module::{ModuleId, instantiation::InstanceId, port::NonAnsiPortId},
        package::{PackageId, PackageImportMember},
        stmt::StmtId,
        subroutine::SubroutineId,
        typedef::TypedefId,
    },
    scope::PackageImportEntry,
    semantics::{Semantics, pathres::PathResolution},
    source_map::{IsNamedSrc, IsSrc},
};
use ide_db::root_db::RootDb;
use smallvec::{SmallVec, smallvec};
use smol_str::SmolStr;
use syntax::{SyntaxTokenWithParent, ast, match_ast, token::TokenKindExt};
use utils::{
    get::{Get, GetRef},
    impl_from,
    line_index::{TextRange, TextSize},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum DefinitionOrigin {
    ModuleId(ModuleId),
    BlockId(BlockId),

    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
    Typedef(InContainer<TypedefId>),
    Class(InContainer<ClassId>),
    Package(PackageId),
    PackageImport(InModule<PackageImportEntry>),
    Subroutine(InContainer<SubroutineId>),
}

impl_from! { DefinitionOrigin =>
    ModuleId,
    BlockId,
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
    Typedef(InContainer<TypedefId>),
    Class(InContainer<ClassId>),
    Package(PackageId),
    PackageImport(InModule<PackageImportEntry>),
    Subroutine(InContainer<SubroutineId>),
}

impl DefinitionOrigin {
    #[inline]
    pub fn container_id(&self, db: &dyn HirDb) -> ContainerId {
        match *self {
            DefinitionOrigin::ModuleId(InFile { file_id, .. }) => file_id.into(),
            DefinitionOrigin::BlockId(block_id) => block_id.lookup(db).cont_id,
            DefinitionOrigin::NonAnsiPort(InModule { module_id, .. }) => module_id.into(),
            DefinitionOrigin::Decl(InContainer { cont_id, .. }) => cont_id,
            DefinitionOrigin::Instance(InModule { module_id, .. }) => module_id.into(),
            DefinitionOrigin::Stmt(InContainer { cont_id, .. }) => cont_id,
            DefinitionOrigin::Typedef(InContainer { cont_id, .. }) => cont_id,
            DefinitionOrigin::Class(InContainer { cont_id, .. }) => cont_id,
            DefinitionOrigin::Package(InFile { file_id, .. }) => file_id.into(),
            DefinitionOrigin::PackageImport(InModule { module_id, .. }) => module_id.into(),
            DefinitionOrigin::Subroutine(InContainer { cont_id, .. }) => cont_id,
        }
    }

    pub fn name(&self, db: &dyn HirDb) -> SmolStr {
        match *self {
            DefinitionOrigin::ModuleId(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone().unwrap()
            }
            DefinitionOrigin::BlockId(block_id) => {
                let BlockLoc { cont_id, src: InFile { value, file_id: _ } } = block_id.lookup(db);
                let cont = cont_id.to_container(db);
                value.hir(&cont, &cont_id.to_container_src_map(db)).name.clone().unwrap()
            }
            DefinitionOrigin::NonAnsiPort(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).label.clone().unwrap()
            }
            DefinitionOrigin::Decl(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone().unwrap()
            }
            DefinitionOrigin::Instance(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone().unwrap()
            }
            DefinitionOrigin::Stmt(InContainer { value, cont_id }) => cont_id
                .to_container(db)
                .get(value)
                .label
                .clone()
                .unwrap_or_else(|| DEFAULT_NAME.clone()),
            DefinitionOrigin::Typedef(InContainer { value, cont_id }) => cont_id
                .to_container(db)
                .get(value)
                .name
                .clone()
                .unwrap_or_else(|| DEFAULT_NAME.clone()),
            DefinitionOrigin::Class(InContainer { value, cont_id }) => match cont_id {
                ContainerId::HirFileId(file_id) => {
                    let file = file_id.to_container(db);
                    file.classes.get(value).name.clone().unwrap_or_else(|| DEFAULT_NAME.clone())
                }
                ContainerId::ModuleId(module_id) => {
                    let module = module_id.to_container(db);
                    module.classes.get(value).name.clone().unwrap_or_else(|| DEFAULT_NAME.clone())
                }
                ContainerId::PackageId(package_id) => {
                    let pkg = package_id.to_container(db);
                    pkg.classes.get(value).name.clone().unwrap_or_else(|| DEFAULT_NAME.clone())
                }
                ContainerId::BlockId(_) => DEFAULT_NAME.clone(),
                ContainerId::SubroutineId(_) => DEFAULT_NAME.clone(),
                ContainerId::FileSubroutineId(_) => DEFAULT_NAME.clone(),
            },
            DefinitionOrigin::Subroutine(InContainer { value, cont_id }) => match cont_id {
                ContainerId::ModuleId(module_id) => {
                    let module = module_id.to_container(db);
                    module
                        .subroutines
                        .get(value)
                        .name
                        .clone()
                        .unwrap_or_else(|| DEFAULT_NAME.clone())
                }
                ContainerId::PackageId(package_id) => {
                    let package = package_id.to_container(db);
                    package
                        .subroutines
                        .get(value)
                        .name
                        .clone()
                        .unwrap_or_else(|| DEFAULT_NAME.clone())
                }
                ContainerId::HirFileId(file_id) => {
                    let file = file_id.to_container(db);
                    file.subroutines.get(value).name.clone().unwrap_or_else(|| DEFAULT_NAME.clone())
                }
                ContainerId::BlockId(_) => DEFAULT_NAME.clone(),
                ContainerId::SubroutineId(loc) => {
                    let subroutine = db.subroutine(loc);
                    subroutine.name.clone().unwrap_or_else(|| DEFAULT_NAME.clone())
                }
                ContainerId::FileSubroutineId(InFile { file_id, value: sub_id }) => {
                    let file = file_id.to_container(db);
                    file.subroutines
                        .get(sub_id)
                        .name
                        .clone()
                        .unwrap_or_else(|| DEFAULT_NAME.clone())
                }
            },
            DefinitionOrigin::Package(InFile { value, file_id }) => file_id
                .to_container(db)
                .packages
                .get(value)
                .name
                .clone()
                .unwrap_or_else(|| DEFAULT_NAME.clone()),
            DefinitionOrigin::PackageImport(InModule { value, module_id }) => {
                let module = module_id.to_container(db);
                let import = module.package_imports.get(value.import);
                let Some(item) = import.items.get(value.item_idx as usize) else {
                    return DEFAULT_NAME.clone();
                };
                match &item.member {
                    PackageImportMember::Named(name) => name.clone(),
                    PackageImportMember::All => SmolStr::from(format!("{}::*", item.package)),
                }
            }
        }
    }

    pub fn name_range(&self, db: &dyn HirDb) -> InFile<TextRange> {
        match *self {
            DefinitionOrigin::ModuleId(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::BlockId(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.name_range().unwrap();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(module_id.file_id, range)
            }
            DefinitionOrigin::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(module_id.file_id, range)
            }
            DefinitionOrigin::Stmt(InContainer { value, cont_id }) => {
                let src = cont_id.to_container_src_map(db).get(value);
                let range = src.name_range().unwrap_or_else(|| src.range());
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Typedef(InContainer { value, cont_id }) => {
                let src = cont_id.to_container_src_map(db).get(value);
                let range = src.name_range().unwrap_or_else(|| src.range());
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Class(InContainer { value, cont_id }) => match cont_id {
                ContainerId::HirFileId(file_id) => {
                    let src = file_id.to_container_src_map(db).get(value);
                    let range = src.name_range().unwrap_or_else(|| src.range());
                    InFile::new(file_id, range)
                }
                ContainerId::ModuleId(module_id) => {
                    let src = module_id.to_container_src_map(db).get(value);
                    let range = src.name_range().unwrap_or_else(|| src.range());
                    InFile::new(module_id.file_id, range)
                }
                ContainerId::PackageId(package_id) => {
                    let src = package_id.to_container_src_map(db).get(value);
                    let range = src.name_range().unwrap_or_else(|| src.range());
                    InFile::new(package_id.file_id, range)
                }
                ContainerId::BlockId(block_id) => InFile::new(
                    HirFileId(block_id.file_id(db)),
                    TextRange::empty(TextSize::from(0)),
                ),
                ContainerId::SubroutineId(loc) => InFile::new(
                    HirFileId(loc.module_id.file_id()),
                    TextRange::empty(TextSize::from(0)),
                ),
                ContainerId::FileSubroutineId(InFile { file_id, .. }) => {
                    InFile::new(file_id, TextRange::empty(TextSize::from(0)))
                }
            },
            DefinitionOrigin::PackageImport(InModule { value, module_id }) => {
                let src = module_id.to_container_src_map(db).get(value.import);
                InFile::new(module_id.file_id, src.range())
            }
            DefinitionOrigin::Subroutine(InContainer { value, cont_id }) => match cont_id {
                ContainerId::ModuleId(module_id) => {
                    let src = module_id.to_container_src_map(db).get(value);
                    InFile::new(module_id.file_id, src.range())
                }
                ContainerId::PackageId(package_id) => {
                    let src = package_id.to_container_src_map(db).get(value);
                    InFile::new(package_id.file_id, src.range())
                }
                ContainerId::BlockId(block_id) => {
                    let file_id = HirFileId(block_id.file_id(db));
                    InFile::new(file_id, TextRange::empty(TextSize::from(0)))
                }
                ContainerId::HirFileId(file_id) => {
                    InFile::new(file_id, TextRange::empty(TextSize::from(0)))
                }
                ContainerId::SubroutineId(loc) => {
                    let module_id = loc.module_id;
                    let src = module_id.to_container_src_map(db).get(loc.value);
                    InFile::new(module_id.file_id, src.range())
                }
                ContainerId::FileSubroutineId(InFile { file_id, value: sub_id }) => {
                    let src = file_id.to_container_src_map(db).get(sub_id);
                    InFile::new(file_id, src.range())
                }
            },
            DefinitionOrigin::Package(InFile { value, file_id }) => {
                let src = file_id.to_container_src_map(db).get(value);
                let range = src.name_range().unwrap_or_else(|| src.range());
                InFile::new(file_id, range)
            }
        }
    }

    pub fn range(&self, db: &dyn HirDb) -> InFile<TextRange> {
        match *self {
            DefinitionOrigin::ModuleId(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value).range();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::BlockId(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value).range();
                InFile::new(module_id.file_id, range)
            }
            DefinitionOrigin::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value).range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value).range();
                InFile::new(module_id.file_id, range)
            }
            DefinitionOrigin::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value).range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value).range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Class(InContainer { value, cont_id }) => match cont_id {
                ContainerId::HirFileId(file_id) => {
                    let range = file_id.to_container_src_map(db).get(value).range();
                    InFile::new(file_id, range)
                }
                ContainerId::ModuleId(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value).range();
                    InFile::new(module_id.file_id, range)
                }
                ContainerId::PackageId(package_id) => {
                    let range = package_id.to_container_src_map(db).get(value).range();
                    InFile::new(package_id.file_id, range)
                }
                ContainerId::BlockId(block_id) => {
                    let BlockLoc { src: InFile { value: block_src, file_id }, .. } =
                        block_id.lookup(db);
                    InFile::new(file_id, block_src.range())
                }
                ContainerId::SubroutineId(loc) => InFile::new(
                    HirFileId(loc.module_id.file_id()),
                    TextRange::empty(TextSize::from(0)),
                ),
                ContainerId::FileSubroutineId(InFile { file_id, .. }) => {
                    InFile::new(file_id, TextRange::empty(TextSize::from(0)))
                }
            },
            DefinitionOrigin::Package(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value).range();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::PackageImport(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value.import).range();
                InFile::new(module_id.file_id, range)
            }
            DefinitionOrigin::Subroutine(InContainer { value, cont_id }) => match cont_id {
                ContainerId::ModuleId(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value).range();
                    InFile::new(module_id.file_id, range)
                }
                ContainerId::PackageId(package_id) => {
                    let range = package_id.to_container_src_map(db).get(value).range();
                    InFile::new(package_id.file_id, range)
                }
                ContainerId::BlockId(block_id) => {
                    let file_id = HirFileId(block_id.file_id(db));
                    InFile::new(file_id, TextRange::empty(TextSize::from(0)))
                }
                ContainerId::HirFileId(file_id) => {
                    InFile::new(file_id, TextRange::empty(TextSize::from(0)))
                }
                ContainerId::SubroutineId(loc) => {
                    let module_id = loc.module_id;
                    let range = module_id.to_container_src_map(db).get(loc.value).range();
                    InFile::new(module_id.file_id, range)
                }
                ContainerId::FileSubroutineId(InFile { file_id, value: sub_id }) => {
                    let range = file_id.to_container_src_map(db).get(sub_id).range();
                    InFile::new(file_id, range)
                }
            },
        }
    }
}

// Definition may have multiple origins, e.g. non-ansi port
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition(pub PathResolution);

impl From<PathResolution> for Definition {
    fn from(res: PathResolution) -> Self {
        Self(res)
    }
}

impl Definition {
    pub fn origins(&self) -> SmallVec<[DefinitionOrigin; 3]> {
        let mut res = smallvec![];
        let mut add_source = |source| res.push(source);

        match self.0 {
            PathResolution::NonAnsiPort { label, port_decl, data_decl, module } => {
                let container: ContainerId = module.into();
                if let Some(label) = label {
                    add_source(InModule::new(module, label).into());
                }
                if let Some(port_decl) = port_decl {
                    add_source(InContainer::new(container, port_decl).into());
                }
                if let Some(decl) = data_decl {
                    add_source(InContainer::new(container, decl).into());
                }
            }
            _ => add_source(self.pick()),
        };

        res
    }

    pub fn declaration_origins(&self) -> Option<DefinitionOrigin> {
        match self.0 {
            PathResolution::NonAnsiPort { port_decl, data_decl, module, .. } => {
                let container: ContainerId = module.into();
                if let Some(port_decl) = port_decl {
                    Some(InContainer::new(container, port_decl).into())
                } else {
                    data_decl.map(|decl| InContainer::new(container, decl).into())
                }
            }
            _ => Some(self.pick()),
        }
    }

    pub fn def_origins(&self) -> SmallVec<[DefinitionOrigin; 2]> {
        let mut res = SmallVec::new();
        match self.0 {
            PathResolution::NonAnsiPort { port_decl, data_decl, module, .. } => {
                let container: ContainerId = module.into();
                if let Some(port_decl) = port_decl {
                    res.push(InContainer::new(container, port_decl).into());
                }

                if let Some(decl) = data_decl {
                    res.push(InContainer::new(container, decl).into());
                }
            }
            _ => res.push(self.pick()),
        }

        res
    }

    pub fn is_port(&self) -> bool {
        matches!(self.0, PathResolution::AnsiPort(_) | PathResolution::NonAnsiPort { .. })
    }

    pub fn container_id(&self, db: &dyn HirDb) -> ContainerId {
        let container_id = self.pick().container_id(db);
        debug_assert! {
            self.origins().into_iter().all(|source| source.container_id(db) == container_id)
        };
        container_id
    }

    #[inline]
    fn pick(&self) -> DefinitionOrigin {
        match self.0 {
            PathResolution::Module(module_id) => module_id.into(),
            PathResolution::Decl(decl_id) => decl_id.into(),
            PathResolution::Instance(instance_id) => instance_id.into(),
            PathResolution::Stmt(stmt_id) => stmt_id.into(),
            PathResolution::Block(blk_id) => blk_id.into(),
            PathResolution::ParamDecl(decl_id) | PathResolution::AnsiPort(decl_id) => {
                InContainer::new(decl_id.module_id.into(), decl_id.value).into()
            }
            PathResolution::NonAnsiPort { label, port_decl, data_decl, module } => {
                let container: ContainerId = module.into();
                if let Some(label) = label {
                    InModule::new(module, label).into()
                } else if let Some(port_decl) = port_decl {
                    InContainer::new(container, port_decl).into()
                } else if let Some(decl) = data_decl {
                    InContainer::new(container, decl).into()
                } else {
                    unreachable!(
                        "NonAnsiPort should have at least one of label, port_decl, data_decl"
                    )
                }
            }
            PathResolution::Typedef(typedef) => typedef.into(),
            PathResolution::Class(class_id) => class_id.into(),
            PathResolution::PackageImport(import) => import.into(),
            PathResolution::Package(package) => package.into(),
            PathResolution::Subroutine(sub) => sub.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionClass {
    Definition(Definition),
    PortConnShorthand { port: Definition, local: Definition },
}

impl_from! { DefinitionClass =>
    Definition,
}

impl DefinitionClass {
    pub(crate) fn resolve(
        sema: &Semantics<'_, RootDb>,
        tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
    ) -> Option<Self> {
        if !tok.kind().name_like() {
            return None;
        }

        let res = match_ast! { parent,
            ast::MemberAccessExpression => unimplemented!(),
            ast::ScopedName => unimplemented!(),
            ast::NamedParamAssignment[it] if it.name() == Some(tok) => {
                sema.nameres_named_param_assign(it).map(Definition::from)?.into()
            },
            ast::NamedPortConnection[it] if it.name() == Some(tok) => {
                let port = sema.nameres_named_port_conn(it).map(Definition::from);

                if it.open_paren().is_none() && it.close_paren().is_none() {
                    let local = sema.nameres_ident(tp).map(Definition::from);

                    match (port, local) {
                        (Some(port), Some(local)) => Self::PortConnShorthand { port, local },
                        (Some(it), None) | (None, Some(it)) => it.into(),
                        (None, None) => return None,
                    }
                } else {
                    port?.into()
                }
            },
            _ => Definition::from(sema.nameres_ident(tp)?).into(),
        };

        Some(res)
    }

    pub(crate) fn origins(self) -> SmallVec<[DefinitionOrigin; 6]> {
        match self {
            DefinitionClass::Definition(definition) => definition.origins().into_iter().collect(),
            DefinitionClass::PortConnShorthand { port, local } => {
                port.origins().into_iter().chain(local.origins()).collect()
            }
        }
    }
}
