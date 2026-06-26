use hir::{
    def_id::{ModuleDefId, ModuleDefOrigin},
    file::HirFileId,
    semantics::{Semantics, pathres::PathResolution},
};
use smallvec::SmallVec;
use syntax::{
    SyntaxAncestors, SyntaxToken, SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_name::HasName,
    match_ast,
    token::TokenKindExt,
};

use crate::{
    db::root_db::RootDb,
    module_resolution::{
        ModuleResolution, resolve_instantiation_target, resolve_named_param_assignment,
        resolve_named_port_connection,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionClass {
    Definition(ModuleDefId),
    PortConnShorthand { port: ModuleDefId, local: ModuleDefId },
    Ambiguous(Vec<ModuleDefId>),
}

impl From<ModuleDefId> for DefinitionClass {
    fn from(def: ModuleDefId) -> Self {
        Self::Definition(def)
    }
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
                    .and_then(|res| res.to_def_id(sema.db))?.into()
            },
            ast::NamedPortConnection[it] if it.name() == Some(tok) => {
                let port = resolve_named_port_connection(sema.db, file_id.file_id(), it)
                    .and_then(|res| res.to_def_id(sema.db));

                if it.open_paren().is_none() && it.close_paren().is_none() {
                    let local = sema.nameres_ident(file_id, tp).and_then(|res| res.to_def_id(sema.db));

                    match (port, local) {
                        (Some(port), Some(local)) => Self::PortConnShorthand { port, local },
                        (Some(it), None) | (None, Some(it)) => it.into(),
                        (None, None) => return None,
                    }
                } else {
                    port?.into()
                }
            },
            _ => sema.nameres_ident(file_id, tp)?.to_def_id(sema.db)?.into(),
        };

        Some(res)
    }

    pub(crate) fn origins(self, db: &RootDb) -> SmallVec<[ModuleDefOrigin; 6]> {
        match self {
            DefinitionClass::Definition(definition) => definition.origins(db).into_iter().collect(),
            DefinitionClass::PortConnShorthand { port, local } => {
                port.origins(db).into_iter().chain(local.origins(db)).collect()
            }
            DefinitionClass::Ambiguous(definitions) => {
                definitions.into_iter().flat_map(|definition| definition.origins(db)).collect()
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
        return Some(PathResolution::Module(module_id).to_def_id(sema.db)?.into());
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
        return Some(res.to_def_id(sema.db)?.into());
    }

    let scoped = SyntaxAncestors::start_from(parent).find_map(ast::ScopedName::cast)?;
    let right_tok = scoped_right_token(scoped)?;
    if right_tok != tok {
        return None;
    }

    let expr = ast::Expression::cast(scoped.syntax())?;
    let res = sema.expr_to_def(sema.resolve_expr(file_id, expr)?)?;
    Some(res.to_def_id(sema.db)?.into())
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
                Some(PathResolution::Module(module_id).to_def_id(sema.db)?.into())
            }
            ModuleResolution::Ambiguous { candidates, .. } => Some(DefinitionClass::Ambiguous(
                candidates
                    .into_iter()
                    .map(|module_id| PathResolution::Module(module_id).to_def_id(sema.db))
                    .collect::<Option<Vec<_>>>()?,
            )),
            ModuleResolution::Unresolved => None,
        };
    }

    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::PrimitiveInstantiation::cast)
        && instantiation.type_() == Some(tok)
    {
        return Some(sema.nameres_ident(file_id, tp)?.to_def_id(sema.db)?.into());
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
        def_id::ModuleDefOrigin,
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

            let origins = def.origins(db);
            let (resolution, range) = match origins.first().copied() {
                Some(origin @ ModuleDefOrigin::NonAnsiPort(_)) => (
                    "NonAnsiPort",
                    origin.name_range(db).expect("non-ANSI port label should have a name range"),
                ),
                Some(origin @ ModuleDefOrigin::Decl(_)) => {
                    ("AnsiPort", origin.name_range(db).expect("ANSI port should have a name range"))
                }
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
