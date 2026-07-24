use hir::{
    db::HirDb,
    def_id::DefId,
    file::HirFileId,
    hir_def::lower_ident_opt,
    semantics::Semantics,
    symbol::{DefKind, DefOrigin, NameContext, Resolution},
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DefinitionClass {
    Definition(DefId),
    PortConnShorthand { port: DefId, local: DefId },
}

pub type DefinitionResolution = Resolution<DefinitionClass>;

impl DefinitionClass {
    pub(crate) fn resolve(
        sema: &Semantics<'_, RootDb>,
        file_id: HirFileId,
        tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
    ) -> Option<DefinitionResolution> {
        if !tok.kind().name_like() {
            return None;
        }

        if let Some(resolution) = resolve_member_or_scoped_name(sema, file_id, tp) {
            return Some(resolution);
        }

        if let Some(resolution) = resolve_declaration_name(sema, file_id, tp) {
            return Some(resolution);
        }

        if let Some(resolution) = resolve_instantiation_type_name(sema, file_id, tp) {
            return Some(resolution);
        }

        if let Some(resolution) = resolve_package_import_item(sema, file_id, tp) {
            return Some(resolution);
        }

        if let Some(resolution) = resolve_package_scoped_name(sema, file_id, tp) {
            return Some(resolution);
        }

        if token_is_in_non_dot_scoped_name(parent) {
            return None;
        }

        let resolution = match_ast! { parent,
            ast::NamedParamAssignment[it] if it.name() == Some(tok) => {
                resolve_named_param_assignment(sema.db, file_id.file_id(), it)?
                    .map(DefinitionClass::Definition)
            },
            ast::NamedPortConnection[it] if it.name() == Some(tok) => {
                let port = resolve_named_port_connection(sema.db, file_id.file_id(), it)
                    .unwrap_or(Resolution::Unresolved);

                if it.open_paren().is_none() && it.close_paren().is_none() {
                    let local = sema
                        .nameres_ident(file_id, tp, NameContext::Value)
                        .unwrap_or(Resolution::Unresolved);
                    combine_port_shorthand(port, local)
                } else {
                    port.map(DefinitionClass::Definition)
                }
            },
            _ => sema
                .nameres_ident(file_id, tp, name_context_for_token(parent))?
                .map(DefinitionClass::Definition),
        };

        resolution.into_option()
    }

    pub(crate) fn origins(self, db: &RootDb) -> SmallVec<[DefOrigin; 6]> {
        match self {
            DefinitionClass::Definition(definition) => definition.origins(db).into_iter().collect(),
            DefinitionClass::PortConnShorthand { port, local } => {
                port.origins(db).into_iter().chain(local.origins(db)).collect()
            }
        }
    }
}

fn combine_port_shorthand(
    port: Resolution<DefId>,
    local: Resolution<DefId>,
) -> DefinitionResolution {
    match (&port, &local) {
        (Resolution::Unresolved, Resolution::Unresolved) => Resolution::Unresolved,
        (Resolution::Unresolved, _) => local.map(DefinitionClass::Definition),
        (_, Resolution::Unresolved) => port.map(DefinitionClass::Definition),
        _ => Resolution::from_candidates(port.into_candidates().into_iter().flat_map(|port| {
            local
                .candidates()
                .iter()
                .copied()
                .map(move |local| DefinitionClass::PortConnShorthand { port, local })
        })),
    }
}

fn resolve_declaration_name(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionResolution> {
    if let Some(module) = SyntaxAncestors::start_from(parent).find_map(ast::ModuleDeclaration::cast)
        && module.name() == Some(tok)
    {
        let module_id = sema.module_to_def(file_id, module)?;
        return Some(Resolution::Unique(DefinitionClass::Definition(DefId::new(
            sema.db, module_id,
        ))));
    }

    None
}

fn resolve_member_or_scoped_name(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionResolution> {
    if let Some(access) =
        SyntaxAncestors::start_from(parent).find_map(ast::MemberAccessExpression::cast)
        && access.name() == Some(tok)
    {
        let expr = ast::Expression::cast(access.syntax())?;
        return Some(
            sema.expr_to_def(sema.resolve_expr(file_id, expr)?)?.map(DefinitionClass::Definition),
        );
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
    Some(sema.expr_to_def(sema.resolve_expr(file_id, expr)?)?.map(DefinitionClass::Definition))
}

fn resolve_package_scoped_name(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionResolution> {
    let scoped = SyntaxAncestors::start_from(parent).find_map(ast::ScopedName::cast)?;
    if scoped_uses_dot(scoped) {
        return None;
    }

    let left = scoped_left_token(scoped)?;
    let packages = package_defs(sema, file_id, left)?;
    if left.tok == tok {
        return packages.map(DefinitionClass::Definition).into_option();
    }

    let right_tok = scoped_right_token(scoped)?;
    if right_tok != tok {
        return None;
    }

    let ident = lower_ident_opt(Some(tok))?;
    let primary_ctx = name_context_for_token(parent);
    package_member_resolution(sema, packages, &ident, primary_ctx).into_option()
}

fn resolve_package_import_item(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionResolution> {
    let item = SyntaxAncestors::start_from(parent).find_map(ast::PackageImportItem::cast)?;
    let package_token = SyntaxTokenWithParent { parent: item.syntax(), tok: item.package()? };
    let packages = package_defs(sema, file_id, package_token)?;
    if item.package() == Some(tok) {
        return packages.map(DefinitionClass::Definition).into_option();
    }

    if item.item() != Some(tok) {
        return None;
    }
    let ident = lower_ident_opt(Some(tok))?;
    package_member_resolution(sema, packages, &ident, NameContext::Type).into_option()
}

fn package_defs(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    token: SyntaxTokenWithParent<'_>,
) -> Option<Resolution<DefId>> {
    let resolution = sema.nameres_ident(file_id, token, NameContext::Type)?;
    Resolution::from_candidates(
        resolution
            .into_candidates()
            .into_iter()
            .filter(|def| def.kind(sema.db) == DefKind::Package),
    )
    .into_option()
}

fn package_member_resolution(
    sema: &Semantics<'_, RootDb>,
    packages: Resolution<DefId>,
    ident: &hir::hir_def::Ident,
    primary_ctx: NameContext,
) -> DefinitionResolution {
    let fallback_ctx =
        if primary_ctx == NameContext::Type { NameContext::Value } else { NameContext::Type };
    let mut members = SmallVec::<[DefId; 2]>::new();
    for package in packages.into_candidates() {
        let Some(package_id) = package.as_module(sema.db) else {
            continue;
        };
        members.extend(
            sema.db
                .package_export_scope(package_id)
                .lookup(primary_ctx, ident)
                .or_else(|| sema.db.package_export_scope(package_id).lookup(fallback_ctx, ident))
                .into_candidates(),
        );
    }
    Resolution::from_candidates(members).map(DefinitionClass::Definition)
}

fn resolve_instantiation_type_name(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionResolution> {
    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::HierarchyInstantiation::cast)
        && instantiation.type_() == Some(tok)
    {
        let resolution =
            match resolve_instantiation_target(sema.db, file_id.file_id(), instantiation) {
                ModuleResolution::Unique(module_id)
                | ModuleResolution::BestEffortProximity { selected: module_id, .. } => {
                    Resolution::Unique(DefId::new(sema.db, module_id))
                }
                ModuleResolution::Ambiguous { candidates, .. } => Resolution::from_candidates(
                    candidates.into_iter().map(|module_id| DefId::new(sema.db, module_id)),
                ),
                ModuleResolution::Unresolved => {
                    sema.nameres_ident(file_id, tp, NameContext::Type)?
                }
            };
        return resolution.map(DefinitionClass::Definition).into_option();
    }

    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::PrimitiveInstantiation::cast)
        && instantiation.type_() == Some(tok)
    {
        return sema
            .nameres_ident(file_id, tp, NameContext::Value)?
            .map(DefinitionClass::Definition)
            .into_option();
    }

    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::CheckerInstantiation::cast)
        && rightmost_name_token(instantiation.type_()) == Some(tok)
    {
        return sema
            .nameres_ident(file_id, tp, NameContext::Type)?
            .map(DefinitionClass::Definition)
            .into_option();
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
                DefinitionClass::resolve(&sema, file_id.into(), token).unwrap().unique().unwrap()
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
            DefinitionClass::resolve(&sema, file_id.into(), token).unwrap().unique().unwrap()
        else {
            panic!("expected plain definition for hierarchical leaf");
        };

        let origins = def.origins(db);
        assert!(
            origins.iter().any(|origin| origin.kind(db) == DefKind::Net),
            "hierarchical leaf should resolve to child net, got {origins:?}"
        );
    }

    #[test]
    fn ordinary_name_resolution_preserves_ambiguity() {
        let text = r#"
module m;
  wire duplicate;
  wire duplicate;
  wire sink = du/*caret*/plicate;
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

        let resolution = DefinitionClass::resolve(&sema, file_id.into(), token).unwrap();
        let Resolution::Ambiguous(candidates) = resolution else {
            panic!("duplicate declarations should produce an ambiguous definition resolution");
        };
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|candidate| {
            matches!(candidate, DefinitionClass::Definition(def) if def.origins(db).len() == 1)
        }));
    }
}
