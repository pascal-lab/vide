use hir::{
    db::HirDb,
    def_id::ModuleDefId,
    file::HirFileId,
    hir_def::lower_ident_opt,
    semantics::{Semantics, pathres::PathResolution},
    symbol::{DefId, DefKind, NameContext},
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

        if let Some(def) = resolve_package_import_item(sema, file_id, tp) {
            return Some(def);
        }

        if let Some(def) = resolve_package_scoped_name(sema, file_id, tp) {
            return Some(def);
        }

        if token_is_in_non_dot_scoped_name(parent) {
            return None;
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
                    let local = sema
                        .nameres_ident(file_id, tp, NameContext::Value)
                        .and_then(|res| res.to_def_id(sema.db));

                    match (port, local) {
                        (Some(port), Some(local)) => Self::PortConnShorthand { port, local },
                        (Some(it), None) | (None, Some(it)) => it.into(),
                        (None, None) => return None,
                    }
                } else {
                    port?.into()
                }
            },
            _ => sema
                .nameres_ident(file_id, tp, name_context_for_token(parent))
                ?.to_def_id(sema.db)?
                .into(),
        };

        Some(res)
    }

    pub(crate) fn origins(self, db: &RootDb) -> SmallVec<[DefId; 6]> {
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
        return Some(
            PathResolution::from_def_id(DefId::new(sema.db, module_id)).to_def_id(sema.db)?.into(),
        );
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
    if !scoped_uses_dot(scoped) {
        return None;
    }
    let right_tok = scoped_right_token(scoped)?;
    if right_tok != tok {
        return None;
    }

    let expr = ast::Expression::cast(scoped.syntax())?;
    let res = sema.expr_to_def(sema.resolve_expr(file_id, expr)?)?;
    Some(res.to_def_id(sema.db)?.into())
}

fn resolve_package_scoped_name(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionClass> {
    let scoped = SyntaxAncestors::start_from(parent).find_map(ast::ScopedName::cast)?;
    if scoped_uses_dot(scoped) {
        return None;
    }

    let left = scoped_left_token(scoped)?;
    if left.tok == tok {
        return package_def(sema, file_id, left).map(Into::into);
    }

    let right_tok = scoped_right_token(scoped)?;
    if right_tok != tok {
        return None;
    }

    let package_def = package_def(sema, file_id, left)?;
    let package_id = package_id_from_def(sema, package_def)?;
    let ident = lower_ident_opt(Some(tok))?;
    let primary_ctx = name_context_for_token(parent);
    package_member_def(sema, package_id, &ident, primary_ctx).map(Into::into)
}

fn resolve_package_import_item(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionClass> {
    let item = SyntaxAncestors::start_from(parent).find_map(ast::PackageImportItem::cast)?;
    if item.package() == Some(tok) {
        return package_def(sema, file_id, tp).map(Into::into);
    }

    if item.item() != Some(tok) {
        return None;
    }
    let package_tok = item.package()?;
    let package_def = package_def(
        sema,
        file_id,
        SyntaxTokenWithParent { parent: item.syntax(), tok: package_tok },
    )?;
    let package_id = package_id_from_def(sema, package_def)?;
    let ident = lower_ident_opt(Some(tok))?;
    package_member_def(sema, package_id, &ident, NameContext::Type).map(Into::into)
}

fn package_def(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    left: SyntaxTokenWithParent<'_>,
) -> Option<ModuleDefId> {
    sema.nameres_ident(file_id, left, NameContext::Type)?
        .def_ids()
        .iter()
        .copied()
        .find(|def| def.kind(sema.db) == DefKind::Package)
        .and_then(|def| PathResolution::from_def_id(def).to_def_id(sema.db))
}

fn package_id_from_def(
    sema: &Semantics<'_, RootDb>,
    package_def: ModuleDefId,
) -> Option<hir::hir_def::module::PackageId> {
    package_def.origins(sema.db).into_iter().find_map(|def| {
        (def.kind(sema.db) == DefKind::Package).then(|| def.as_module(sema.db)).flatten()
    })
}

fn package_member_def(
    sema: &Semantics<'_, RootDb>,
    package_id: hir::hir_def::module::PackageId,
    ident: &hir::hir_def::Ident,
    primary_ctx: NameContext,
) -> Option<ModuleDefId> {
    let package_scope = sema.db.package_export_scope(package_id);
    let fallback_ctx =
        if primary_ctx == NameContext::Type { NameContext::Value } else { NameContext::Type };
    let defs = package_scope
        .lookup(primary_ctx, ident)
        .or_else(|| package_scope.lookup(fallback_ctx, ident))?;
    PathResolution::from_def_ids(defs)?.to_def_id(sema.db)
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
            | ModuleResolution::BestEffortProximity { selected: module_id, .. } => Some(
                PathResolution::from_def_id(DefId::new(sema.db, module_id))
                    .to_def_id(sema.db)?
                    .into(),
            ),
            ModuleResolution::Ambiguous { candidates, .. } => Some(DefinitionClass::Ambiguous(
                candidates
                    .into_iter()
                    .map(|module_id| {
                        PathResolution::from_def_id(DefId::new(sema.db, module_id))
                            .to_def_id(sema.db)
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            ModuleResolution::Unresolved => {
                Some(sema.nameres_ident(file_id, tp, NameContext::Type)?.to_def_id(sema.db)?.into())
            }
        };
    }

    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::PrimitiveInstantiation::cast)
        && instantiation.type_() == Some(tok)
    {
        return Some(
            sema.nameres_ident(file_id, tp, NameContext::Value)?.to_def_id(sema.db)?.into(),
        );
    }

    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::CheckerInstantiation::cast)
        && rightmost_name_token(instantiation.type_()) == Some(tok)
    {
        return Some(
            sema.nameres_ident(file_id, tp, NameContext::Type)?.to_def_id(sema.db)?.into(),
        );
    }

    None
}

fn name_context_for_token(parent: syntax::SyntaxNode<'_>) -> NameContext {
    if SyntaxAncestors::start_from(parent).any(|node| ast::NamedType::cast(node).is_some()) {
        NameContext::Type
    } else {
        // Value is the conservative default for identifier references in IDE
        // features; type positions are selected by the syntactic NamedType arm
        // above.
        NameContext::Value
    }
}

fn scoped_right_token(scoped: ast::ScopedName<'_>) -> Option<SyntaxToken<'_>> {
    use ast::Name::*;
    match scoped.right() {
        IdentifierName(ident) => ident.identifier(),
        IdentifierSelectName(ident) => ident.identifier(),
        _ => None,
    }
}

fn scoped_left_token(scoped: ast::ScopedName<'_>) -> Option<SyntaxTokenWithParent<'_>> {
    use ast::Name::*;
    match scoped.left() {
        IdentifierName(ident) => {
            Some(SyntaxTokenWithParent { parent: ident.syntax(), tok: ident.identifier()? })
        }
        IdentifierSelectName(ident) => {
            Some(SyntaxTokenWithParent { parent: ident.syntax(), tok: ident.identifier()? })
        }
        _ => None,
    }
}

fn scoped_uses_dot(scoped: ast::ScopedName<'_>) -> bool {
    scoped
        .syntax()
        .children()
        .filter_map(|elem| elem.as_token())
        .any(|tok| tok.kind() == syntax::Token![.])
}

fn rightmost_name_token(name: ast::Name<'_>) -> Option<SyntaxToken<'_>> {
    use ast::Name::*;
    match name {
        IdentifierName(ident) => ident.identifier(),
        IdentifierSelectName(ident) => ident.identifier(),
        ScopedName(scoped) => rightmost_name_token(scoped.right()),
        _ => None,
    }
}

fn token_is_in_non_dot_scoped_name(parent: syntax::SyntaxNode<'_>) -> bool {
    SyntaxAncestors::start_from(parent)
        .find_map(ast::ScopedName::cast)
        .is_some_and(|scoped| !scoped_uses_dot(scoped))
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use hir::{
        base_db::{change::Change, source_root::SourceRoot},
        symbol::DefKind,
    };
    use syntax::SyntaxNodeExt;
    use utils::text_edit::TextSize;
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{analysis_host::AnalysisHost, db::root_db::RootDb};

    fn host_with_file(text: &str) -> (AnalysisHost, FileId) {
        let file_id = FileId::from_raw(0);
        let path = VfsPath::new_virtual_path("/test.v".to_string());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile::create(file_id, text));

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
                Some(origin) if origin.kind(db) == DefKind::NonAnsiPort => (
                    "NonAnsiPort",
                    origin.name_range(db).expect("non-ANSI port label should have a name range"),
                ),
                Some(origin) if origin.kind(db) == DefKind::Port => {
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

    #[test]
    fn definition_resolves_hierarchical_path_leaf() {
        let text = r#"
module leaf;
  wire leaf_wire;
endmodule

module top;
  leaf u0();
  initial begin
    top.u0.leaf_/*caret*/wire;
  end
endmodule
"#;
        let offset = TextSize::from(text.find("/*caret*/").unwrap() as u32);
        let text = text.replace("/*caret*/", "");
        let (host, file_id) = host_with_file(&text);
        let db = host.raw_db();
        let sema = Semantics::<RootDb>::new(db);
        let parsed_file = sema.parse_file(file_id);
        let file = parsed_file.compilation_unit().unwrap();
        let token = file
            .syntax()
            .token_at_offset(offset)
            .pick_bext_token(crate::goto_definition::token_precedence)
            .unwrap();

        let DefinitionClass::Definition(def) =
            DefinitionClass::resolve(&sema, file_id.into(), token).unwrap()
        else {
            panic!("expected plain definition for hierarchical leaf");
        };

        let origins = def.origins(db);
        assert!(
            origins.iter().any(|origin| origin.kind(db) == DefKind::Net),
            "hierarchical leaf should resolve to child net, got {origins:?}"
        );
    }
}
