use base_db::intern::Lookup;
use hir::{
    container::{ContainerId, InContainer, InFile, InModule, InSubroutine},
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockLoc},
        expr::declarator::DeclId,
        file::config::ConfigDeclId,
        module::{ModuleId, instantiation::InstanceId, port::NonAnsiPortId},
        opaque::OpaqueItemId,
        stmt::StmtId,
        subroutine::{SubroutineId, SubroutinePortId},
        typedef::TypedefId,
    },
    semantics::{Semantics, pathres::PathResolution},
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
};
use ide_db::root_db::RootDb;
use smallvec::{SmallVec, smallvec};
use smol_str::SmolStr;
use syntax::{
    SyntaxAncestors, SyntaxToken, SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    match_ast,
    token::TokenKindExt,
};
use utils::{
    get::{Get, GetRef},
    impl_from,
    line_index::TextRange,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum DefinitionOrigin {
    ModuleId(ModuleId),
    Config(InFile<ConfigDeclId>),
    BlockId(BlockId),
    SubroutineId(SubroutineId),
    SubroutinePort(InSubroutine<SubroutinePortId>),

    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Opaque(InContainer<OpaqueItemId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
}

impl_from! { DefinitionOrigin =>
    ModuleId,
    Config(InFile<ConfigDeclId>),
    BlockId,
    SubroutineId,
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Opaque(InContainer<OpaqueItemId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
}

impl DefinitionOrigin {
    #[inline]
    pub fn container_id(&self, db: &dyn HirDb) -> ContainerId {
        match *self {
            DefinitionOrigin::ModuleId(InFile { file_id, .. }) => file_id.into(),
            DefinitionOrigin::Config(InFile { file_id, .. }) => file_id.into(),
            DefinitionOrigin::BlockId(block_id) => block_id.lookup(db).cont_id,
            DefinitionOrigin::SubroutineId(subroutine_id) => subroutine_id.lookup(db).cont_id,
            DefinitionOrigin::SubroutinePort(InSubroutine { subroutine, .. }) => {
                ContainerId::SubroutineId(subroutine)
            }
            DefinitionOrigin::NonAnsiPort(InModule { module_id, .. }) => module_id.into(),
            DefinitionOrigin::Decl(InContainer { cont_id, .. }) => cont_id,
            DefinitionOrigin::Typedef(InContainer { cont_id, .. }) => cont_id,
            DefinitionOrigin::Opaque(InContainer { cont_id, .. }) => cont_id,
            DefinitionOrigin::Instance(InModule { module_id, .. }) => module_id.into(),
            DefinitionOrigin::Stmt(InContainer { cont_id, .. }) => cont_id,
        }
    }

    pub fn name(&self, db: &dyn HirDb) -> SmolStr {
        match *self {
            DefinitionOrigin::ModuleId(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone().unwrap()
            }
            DefinitionOrigin::Config(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone().unwrap()
            }
            DefinitionOrigin::BlockId(block_id) => {
                let BlockLoc { cont_id, src: InFile { value, file_id: _ } } = block_id.lookup(db);
                let cont = cont_id.to_container(db);
                value.hir(&cont, &cont_id.to_container_src_map(db)).name.clone().unwrap()
            }
            DefinitionOrigin::SubroutineId(subroutine_id) => {
                db.subroutine(subroutine_id).name.clone().unwrap()
            }
            DefinitionOrigin::SubroutinePort(InSubroutine { subroutine, value }) => {
                db.subroutine(subroutine).ports[value.0 as usize].name.clone().unwrap()
            }
            DefinitionOrigin::NonAnsiPort(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).label.clone().unwrap()
            }
            DefinitionOrigin::Decl(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone().unwrap()
            }
            DefinitionOrigin::Typedef(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone().unwrap()
            }
            DefinitionOrigin::Opaque(InContainer { value, cont_id }) => {
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
            DefinitionOrigin::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::BlockId(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.name_range().unwrap();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::SubroutineId(subroutine_id) => {
                let src = subroutine_id.lookup(db).src;
                InFile::new(src.file_id, src.value.name_or_full_range())
            }
            DefinitionOrigin::SubroutinePort(InSubroutine { subroutine, value }) => {
                let src = subroutine.lookup(db).src;
                let tree = db.parse(src.file_id);
                let func = src.value.to_node(&tree).unwrap();
                let ports = func
                    .prototype()
                    .port_list()
                    .map(|ports| ports.ports().children().collect::<Vec<_>>())
                    .unwrap_or_default();
                let port = ports
                    .into_iter()
                    .nth(value.0 as usize)
                    .and_then(|port| port.as_function_port())
                    .unwrap();
                let range = port.declarator().name().unwrap().text_range().unwrap();
                InFile::new(src.file_id, range)
            }
            DefinitionOrigin::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(module_id.file_id, range)
            }
            DefinitionOrigin::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value).name_range().unwrap();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Opaque(InContainer { value, cont_id }) => {
                let src = cont_id.to_container_src_map(db).get(value);
                let range = src.name_or_full_range();
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
            DefinitionOrigin::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value).range();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::BlockId(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            DefinitionOrigin::SubroutineId(subroutine_id) => {
                let src = subroutine_id.lookup(db).src;
                let range = src.value.range();
                InFile::new(src.file_id, range)
            }
            DefinitionOrigin::SubroutinePort(InSubroutine { subroutine, value }) => {
                let src = subroutine.lookup(db).src;
                let tree = db.parse(src.file_id);
                let func = src.value.to_node(&tree).unwrap();
                let ports = func
                    .prototype()
                    .port_list()
                    .map(|ports| ports.ports().children().collect::<Vec<_>>())
                    .unwrap_or_default();
                let port = ports
                    .into_iter()
                    .nth(value.0 as usize)
                    .and_then(|port| port.as_function_port())
                    .unwrap();
                InFile::new(src.file_id, port.syntax().text_range().unwrap())
            }
            DefinitionOrigin::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value).range();
                InFile::new(module_id.file_id, range)
            }
            DefinitionOrigin::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value).range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value).range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefinitionOrigin::Opaque(InContainer { value, cont_id }) => {
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
            PathResolution::Config(config_id) => config_id.into(),
            PathResolution::Decl(decl_id) => decl_id.into(),
            PathResolution::Typedef(typedef_id) => typedef_id.into(),
            PathResolution::Opaque(opaque_id) => opaque_id.into(),
            PathResolution::Instance(instance_id) => instance_id.into(),
            PathResolution::Stmt(stmt_id) => stmt_id.into(),
            PathResolution::Block(blk_id) => blk_id.into(),
            PathResolution::Subroutine(subroutine_id) => subroutine_id.into(),
            PathResolution::SubroutinePort(port_id) => port_id.into(),
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

        if let Some(def) = resolve_member_or_scoped_name(sema, tp) {
            return Some(def);
        }

        let res = match_ast! { parent,
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

fn resolve_member_or_scoped_name(
    sema: &Semantics<'_, RootDb>,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionClass> {
    if let Some(access) =
        SyntaxAncestors::start_from(parent).find_map(ast::MemberAccessExpression::cast)
        && access.name() == Some(tok)
    {
        let expr = ast::Expression::cast(access.syntax())?;
        let res = sema.expr_to_def(sema.resolve_expr(expr))?;
        return Some(Definition::from(res).into());
    }

    let scoped = SyntaxAncestors::start_from(parent).find_map(ast::ScopedName::cast)?;
    let right_tok = scoped_right_token(scoped)?;
    if right_tok != tok {
        return None;
    }

    let expr = ast::Expression::cast(scoped.syntax())?;
    let res = sema.expr_to_def(sema.resolve_expr(expr))?;
    Some(Definition::from(res).into())
}

fn scoped_right_token(scoped: ast::ScopedName<'_>) -> Option<SyntaxToken<'_>> {
    use ast::Name::*;
    match scoped.right() {
        IdentifierName(ident) => ident.identifier(),
        IdentifierSelectName(ident) => ident.identifier(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_root::SourceRoot};
    use hir::{container::InModule, semantics::pathres::PathResolution};
    use ide_db::root_db::RootDb;
    use syntax::SyntaxNodeExt;
    use triomphe::Arc;
    use utils::{lines::LineEnding, text_edit::TextSize};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::analysis_host::AnalysisHost;

    fn host_with_file(text: &str) -> (AnalysisHost, FileId) {
        let file_id = FileId(0);
        let path = VfsPath::new_virtual_path("/test.v".to_string());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(text), LineEnding::Unix),
        });

        let mut host = AnalysisHost::default();
        host.apply_change(change);
        (host, file_id)
    }

    #[test]
    fn implicit_non_ansi_port_origin_uses_header_port_name_range() {
        let text = "module m(a); input a; endmodule";
        let (host, file_id) = host_with_file(text);
        let db = host.raw_db();
        let sema = Semantics::<RootDb>::new(db);
        let file = sema.parse(file_id);
        let port_decl_name_offset = TextSize::from(text.find("input a").unwrap() as u32 + 6);
        let token = file.syntax().token_at_offset(port_decl_name_offset).left_biased().unwrap();
        let DefinitionClass::Definition(def) = DefinitionClass::resolve(&sema, token).unwrap()
        else {
            panic!("expected plain definition");
        };

        let PathResolution::NonAnsiPort { label: Some(label), module, .. } = def.0 else {
            panic!("expected non-ANSI port label resolution");
        };
        let range = DefinitionOrigin::NonAnsiPort(InModule::new(module, label)).name_range(db);
        assert_eq!(range.file_id.file_id(), file_id);
        assert_eq!(range.value, TextRange::new(TextSize::from(9), TextSize::from(10)));
    }
}
