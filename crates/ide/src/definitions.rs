use hir::{
    container::{ContainerId, InContainer, InModule},
    db::HirDb,
    file::HirFileId,
    semantics::{Semantics, pathres::PathResolution},
};
use smallvec::{SmallVec, smallvec};
use syntax::{
    SyntaxAncestors, SyntaxToken, SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_name::HasName,
    match_ast,
    token::TokenKindExt,
};
use utils::impl_from;

pub use crate::facts::symbol::DefinitionOrigin;
use crate::{
    db::root_db::RootDb,
    module_resolution::{
        ModuleResolution, resolve_instantiation_target, resolve_named_param_assignment,
        resolve_named_port_connection,
    },
};

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
            _ => {
                if let Some(origin) = self.pick() {
                    add_source(origin);
                }
            }
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
            _ => self.pick(),
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
            _ => {
                if let Some(origin) = self.pick() {
                    res.push(origin);
                }
            }
        }

        res
    }

    pub fn is_port(&self) -> bool {
        matches!(self.0, PathResolution::AnsiPort(_) | PathResolution::NonAnsiPort { .. })
    }

    pub fn container_id(&self, db: &dyn HirDb) -> Option<ContainerId> {
        let container_id =
            self.pick().map(|origin| origin.container_id(db)).or_else(|| match self.0 {
                PathResolution::NonAnsiPort { module, .. } => Some(module.into()),
                _ => None,
            })?;
        debug_assert! {
            self.origins().into_iter().all(|source| source.container_id(db) == container_id)
        };
        Some(container_id)
    }

    #[inline]
    fn pick(&self) -> Option<DefinitionOrigin> {
        match self.0 {
            PathResolution::Module(module_id) => Some(module_id.into()),
            PathResolution::Config(config_id) => Some(config_id.into()),
            PathResolution::Library(library_id) => Some(library_id.into()),
            PathResolution::Udp(udp_id) => Some(udp_id.into()),
            PathResolution::Decl(decl_id) => Some(decl_id.into()),
            PathResolution::Typedef(typedef_id) => Some(typedef_id.into()),
            PathResolution::Instance(instance_id) => Some(instance_id.into()),
            PathResolution::Stmt(stmt_id) => Some(stmt_id.into()),
            PathResolution::Block(blk_id) => Some(blk_id.into()),
            PathResolution::GenerateBlock(generate_block_id) => Some(generate_block_id.into()),
            PathResolution::Subroutine(subroutine_id) => Some(subroutine_id.into()),
            PathResolution::SubroutinePort(port_id) => Some(port_id.into()),
            PathResolution::ParamDecl(decl_id) | PathResolution::AnsiPort(decl_id) => {
                Some(InContainer::new(decl_id.module_id.into(), decl_id.value).into())
            }
            PathResolution::NonAnsiPort { label, port_decl, data_decl, module } => {
                let container: ContainerId = module.into();
                if let Some(label) = label {
                    Some(InModule::new(module, label).into())
                } else if let Some(port_decl) = port_decl {
                    Some(InContainer::new(container, port_decl).into())
                } else {
                    data_decl.map(|decl| InContainer::new(container, decl).into())
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionClass {
    Definition(Definition),
    PortConnShorthand { port: Definition, local: Definition },
    Ambiguous(Vec<Definition>),
}

impl_from! { DefinitionClass =>
    Definition,
}

impl DefinitionClass {
    pub(crate) fn resolve(
        sema: &Semantics<'_, RootDb>,
        file_id: HirFileId,
        tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
    ) -> Option<Self> {
        if !tok.kind().name_like() {
            return None;
        }

        if let Some(def) = resolve_member_or_scoped_name(sema, file_id, tp) {
            return Some(def);
        }

        if let Some(def) = resolve_declaration_name(sema, file_id, tp) {
            return Some(def);
        }

        if let Some(def) = resolve_instantiation_type_name(sema, file_id, tp) {
            return Some(def);
        }

        let res = match_ast! { parent,
            ast::NamedParamAssignment[it] if it.name() == Some(tok) => {
                resolve_named_param_assignment(sema.db, file_id.file_id(), it)
                    .map(Definition::from)?.into()
            },
            ast::NamedPortConnection[it] if it.name() == Some(tok) => {
                let port = resolve_named_port_connection(sema.db, file_id.file_id(), it)
                    .map(Definition::from);

                if it.open_paren().is_none() && it.close_paren().is_none() {
                    let local = sema.nameres_ident(file_id, tp).map(Definition::from);

                    match (port, local) {
                        (Some(port), Some(local)) => Self::PortConnShorthand { port, local },
                        (Some(it), None) | (None, Some(it)) => it.into(),
                        (None, None) => return None,
                    }
                } else {
                    port?.into()
                }
            },
            _ => Definition::from(sema.nameres_ident(file_id, tp)?).into(),
        };

        Some(res)
    }

    pub(crate) fn origins(self) -> SmallVec<[DefinitionOrigin; 6]> {
        match self {
            DefinitionClass::Definition(definition) => definition.origins().into_iter().collect(),
            DefinitionClass::PortConnShorthand { port, local } => {
                port.origins().into_iter().chain(local.origins()).collect()
            }
            DefinitionClass::Ambiguous(definitions) => {
                definitions.into_iter().flat_map(|definition| definition.origins()).collect()
            }
        }
    }
}

fn resolve_declaration_name(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionClass> {
    if let Some(module) = SyntaxAncestors::start_from(parent).find_map(ast::ModuleDeclaration::cast)
        && module.name() == Some(tok)
    {
        let module_id = sema.module_to_def(file_id, module)?;
        return Some(Definition::from(PathResolution::Module(module_id)).into());
    }

    None
}

fn resolve_member_or_scoped_name(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionClass> {
    if let Some(access) =
        SyntaxAncestors::start_from(parent).find_map(ast::MemberAccessExpression::cast)
        && access.name() == Some(tok)
    {
        let expr = ast::Expression::cast(access.syntax())?;
        let res = sema.expr_to_def(sema.resolve_expr(file_id, expr)?)?;
        return Some(Definition::from(res).into());
    }

    let scoped = SyntaxAncestors::start_from(parent).find_map(ast::ScopedName::cast)?;
    let right_tok = scoped_right_token(scoped)?;
    if right_tok != tok {
        return None;
    }

    let expr = ast::Expression::cast(scoped.syntax())?;
    let res = sema.expr_to_def(sema.resolve_expr(file_id, expr)?)?;
    Some(Definition::from(res).into())
}

fn resolve_instantiation_type_name(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionClass> {
    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::HierarchyInstantiation::cast)
        && instantiation.type_() == Some(tok)
    {
        return match resolve_instantiation_target(sema.db, file_id.file_id(), instantiation) {
            ModuleResolution::Unique(module_id)
            | ModuleResolution::BestEffortProximity { selected: module_id, .. } => {
                Some(Definition::from(PathResolution::Module(module_id)).into())
            }
            ModuleResolution::Ambiguous { candidates, .. } => Some(DefinitionClass::Ambiguous(
                candidates
                    .into_iter()
                    .map(|module_id| Definition::from(PathResolution::Module(module_id)))
                    .collect(),
            )),
            ModuleResolution::Unresolved => None,
        };
    }

    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::PrimitiveInstantiation::cast)
        && instantiation.type_() == Some(tok)
    {
        return Some(Definition::from(sema.nameres_ident(file_id, tp)?).into());
    }

    None
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
    use std::fmt::Write;

    use hir::{
        base_db::{change::Change, source_root::SourceRoot},
        container::InModule,
        semantics::pathres::PathResolution,
    };
    use syntax::SyntaxNodeExt;
    use triomphe::Arc;
    use utils::{lines::LineEnding, text_edit::TextSize};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{analysis_host::AnalysisHost, db::root_db::RootDb};

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

    #[derive(Clone, Copy)]
    enum TokenPick {
        LeftBiased,
        GotoDefinition,
    }

    #[test]
    fn definition_name_range_matrix() {
        let mut report = String::new();

        for (name, text, pick) in [
            (
                "implicit non-ansi port",
                "module m(a); input /*caret*/a; endmodule",
                TokenPick::LeftBiased,
            ),
            (
                "named port connection",
                "module child(input clk); endmodule\n\
                    module top; logic clk; child u(.c/*caret*/lk(clk)); endmodule",
                TokenPick::GotoDefinition,
            ),
        ] {
            let offset = TextSize::from(text.find("/*caret*/").unwrap() as u32);
            let text = text.replace("/*caret*/", "");
            let (host, file_id) = host_with_file(&text);
            let db = host.raw_db();
            let sema = Semantics::<RootDb>::new(db);
            let parsed_file = sema.parse_file(file_id);
            let file = parsed_file.compilation_unit().unwrap();
            let tokens = file.syntax().token_at_offset(offset);
            let token = match pick {
                TokenPick::LeftBiased => tokens.left_biased(),
                TokenPick::GotoDefinition => {
                    tokens.pick_bext_token(crate::goto_definition::token_precedence)
                }
            }
            .unwrap();
            let DefinitionClass::Definition(def) =
                DefinitionClass::resolve(&sema, file_id.into(), token).unwrap()
            else {
                panic!("expected plain definition for {name}");
            };

            let (resolution, range) = match def.0 {
                PathResolution::NonAnsiPort { label: Some(label), module, .. } => (
                    "NonAnsiPort",
                    DefinitionOrigin::NonAnsiPort(InModule::new(module, label))
                        .name_range(db)
                        .expect("non-ANSI port label should have a name range"),
                ),
                PathResolution::AnsiPort(port) => (
                    "AnsiPort",
                    DefinitionOrigin::Decl(port.into())
                        .name_range(db)
                        .expect("ANSI port should have a name range"),
                ),
                other => panic!("unexpected definition for {name}: {other:?}"),
            };
            let range_start = usize::from(range.value.start());
            let range_end = usize::from(range.value.end());

            writeln!(&mut report, "{name}:").unwrap();
            writeln!(&mut report, "  resolution: {resolution}").unwrap();
            writeln!(&mut report, "  same_file: {}", range.file_id.file_id() == file_id).unwrap();
            writeln!(&mut report, "  name_range: {:?}", range.value).unwrap();
            writeln!(&mut report, "  name_text: {:?}", &text[range_start..range_end]).unwrap();
            writeln!(&mut report, "  starts_before_caret: {}", range.value.start() < offset)
                .unwrap();
        }

        insta::assert_snapshot!(report);
    }
}
