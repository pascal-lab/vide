use base_db::intern::Lookup;
use hir::{
    container::{ContainerId, InContainer, InFile, InModule},
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockLoc},
        expr::declarator::DeclId,
        module::{ModuleId, instantiation::InstanceId, port::NonAnsiPortId},
        stmt::StmtId,
    },
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
    line_index::TextRange,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum DefinitionOrigin {
    ModuleId(ModuleId),
    BlockId(BlockId),

    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
}

impl_from! { DefinitionOrigin =>
    ModuleId,
    BlockId,
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
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
            DefinitionOrigin::Stmt(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).label.clone().unwrap()
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
                let range = cont_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(cont_id.file_id(db).into(), range)
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
            // Future PathResolution variants not yet handled in this branch.
            _ => unreachable!("Definition navigation not yet supported for this item type"),
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
