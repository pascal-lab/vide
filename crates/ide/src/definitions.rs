use hir_def::{
    db::HirDefDb,
    def_id::DefId,
    has_source::HasSource,
    owner::OwnerId,
    symbol::{DefKind, DefOrigin, NameContext, Resolution},
};
use hir_semantics::semantics::SemanticsImpl;
use preproc_expand::file::HirFileId;
use smallvec::SmallVec;
use syntax::{
    SyntaxAncestors, SyntaxToken, SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_name::HasName,
    match_ast,
    token::TokenKindExt,
};
use vfs::FileId;

use crate::{
    analysis::AnalysisContext,
    db::workspace_symbol_index_db::WorkspaceSymbolIndexDb,
    module_resolution::{resolve_named_param_assignment, resolve_named_port_connection},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionClass {
    Definition(DefId),
    PortConnShorthand { port: DefId, local: DefId },
}

pub type DefinitionResolution = Resolution<DefinitionClass>;

impl DefinitionClass {
    pub(crate) fn resolve(
        db: &AnalysisContext<'_>,
        file_id: HirFileId,
        tp: SyntaxTokenWithParent,
    ) -> DefinitionResolution {
        if let Some(resolution) = resolve_declaration_name_on_db(db.db, file_id, tp) {
            return resolution;
        }
        // `::` is the compilation. Do not map it through ResolutionContext.
        if colon_colon_query(tp).is_some() {
            return Resolution::Unresolved;
        }
        Self::resolve_in(db.db, db.resolution(), file_id, tp, None)
    }

    /// Like [`resolve`](Self::resolve), but resolves identifiers inside a
    /// caller-provided container instead of re-walking the ancestor chain.
    /// The container must be the token's containing scope; callers that walk
    /// the tree (a reference or call-hierarchy walk) track it incrementally.
    pub(crate) fn resolve_in(
        db: &dyn WorkspaceSymbolIndexDb,
        context: triomphe::Arc<hir_def::pathres::ResolutionContext>,
        file_id: HirFileId,
        tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
        container: Option<OwnerId>,
    ) -> DefinitionResolution {
        let sema = SemanticsImpl::new_with_context(db, context.clone());

        if !tok.kind().name_like() {
            return Resolution::Unresolved;
        }

        if let Some(resolution) = resolve_member_or_scoped_name(&sema, file_id, tp) {
            return resolution;
        }

        if let Some(resolution) = resolve_declaration_name(&sema, file_id, tp) {
            return resolution;
        }

        if let Some(resolution) =
            resolve_instantiation_type_name(&context, &sema, file_id, tp, container)
        {
            return resolution;
        }

        if token_is_in_non_dot_scoped_name(parent)
            || SyntaxAncestors::start_from(parent).find_map(ast::PackageImportItem::cast).is_some()
        {
            return Resolution::Unresolved;
        }

        match_ast! { parent,
            ast::NamedParamAssignment[it] if it.name() == Some(tok) => {
                let Some(file) = file_id.source_file_id(db) else {
                    return Resolution::Unresolved;
                };
                resolve_named_param_assignment(db, &context, file, it)
                    .map(DefinitionClass::Definition)
            },
            ast::NamedPortConnection[it] if it.name() == Some(tok) => {
                let Some(file) = file_id.source_file_id(db) else {
                    return Resolution::Unresolved;
                };
                let port = resolve_named_port_connection(db, &context, file, it);

                if it.open_paren().is_none() && it.close_paren().is_none() {
                    let local = nameres_ident(&sema, file_id, tp, NameContext::Value, container);
                    combine_port_shorthand(port, local)
                } else {
                    port.map(DefinitionClass::Definition)
                }
            },
            _ => nameres_ident(&sema, file_id, tp, name_context_for_token(parent), container)
                .map(DefinitionClass::Definition),
        }
    }

    pub(crate) fn origins(self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 6]> {
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
                .cloned()
                .map(move |local| DefinitionClass::PortConnShorthand { port, local })
        })),
    }
}

fn nameres_ident(
    sema: &SemanticsImpl,
    file_id: HirFileId,
    tp: SyntaxTokenWithParent<'_>,
    name_ctx: NameContext,
    container: Option<OwnerId>,
) -> Resolution<DefId> {
    match container {
        Some(container) => sema.nameres_ident_in(file_id, tp, name_ctx, container),
        None => sema.nameres_ident(file_id, tp, name_ctx),
    }
}

fn resolve_declaration_name_on_db(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionResolution> {
    if let Some(module) = SyntaxAncestors::start_from(parent).find_map(ast::ModuleDeclaration::cast)
        && module.name() == Some(tok)
    {
        let resolution = module_declaration_owner(db, file_id, module)
            .map(|module_id| {
                DefinitionClass::Definition(
                    DefId::from_owner(db, module_id).expect("module owner must have a definition"),
                )
            })
            .map(Resolution::Unique)
            .unwrap_or(Resolution::Unresolved);
        return Some(resolution);
    }

    None
}

fn module_declaration_owner(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    module: ast::ModuleDeclaration<'_>,
) -> Option<OwnerId> {
    let tree = db.parse(file_id);
    let ast_id = db.ast_id_map(file_id).id_of_node_in_tree(&tree, module.syntax())?;
    db.owner_table(file_id).owner_by_ast(ast_id, hir_def::owner::OwnerKind::Module)
}

fn resolve_declaration_name(
    sema: &SemanticsImpl,
    file_id: HirFileId,
    tp: SyntaxTokenWithParent,
) -> Option<DefinitionResolution> {
    resolve_declaration_name_on_db(sema.db, file_id, tp)
}

fn resolve_member_or_scoped_name(
    sema: &SemanticsImpl,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<DefinitionResolution> {
    if let Some(access) =
        SyntaxAncestors::start_from(parent).find_map(ast::MemberAccessExpression::cast)
        && access.name() == Some(tok)
    {
        let resolution = ast::Expression::cast(access.syntax())
            .and_then(|expr| sema.resolve_expr(file_id, expr))
            .map(|expr_id| sema.expr_to_def(expr_id))
            .unwrap_or(Resolution::Unresolved);
        return Some(resolution.map(DefinitionClass::Definition));
    }

    let scoped = SyntaxAncestors::start_from(parent).find_map(ast::ScopedName::cast)?;
    if !scoped_uses_dot(scoped) {
        return None;
    }
    let right_tok = scoped_right_token(scoped)?;
    if right_tok != tok {
        return None;
    }

    let resolution = ast::Expression::cast(scoped.syntax())
        .and_then(|expr| sema.resolve_expr(file_id, expr))
        .map(|expr_id| sema.expr_to_def(expr_id))
        .unwrap_or(Resolution::Unresolved);
    Some(resolution.map(DefinitionClass::Definition))
}

pub(crate) fn slang_colon_colon(
    db: &AnalysisContext<'_>,
    file_id: HirFileId,
    tp: SyntaxTokenWithParent<'_>,
) -> Option<DefinitionResolution> {
    use syntax::SyntaxNodeExt;

    let file = file_id.as_file()?;
    let (left, right) = colon_colon_query(tp)?;
    let crate::compile::QueryStatus::Ready(Some(info)) =
        crate::elab_lookup::lookup_scoped_at(db, file, &left, &right)
    else {
        return None;
    };
    if info.def_file.is_empty() {
        return None;
    }
    let origin_file = crate::anchor::file_id_for_slang_path(db.db, &info.def_file);
    let offset = utils::line_index::TextSize::from(info.def_offset as u32);
    let tree = db.parse_file(origin_file);
    let token =
        tree.root().token_at_offset(offset).pick_best_token(crate::token::navigation_precedence)?;
    let resolution =
        DefinitionClass::resolve_in(db.db, db.resolution(), origin_file.into(), token, None);
    (!resolution.is_unresolved()).then_some(resolution)
}

/// Names the compilation answers: types, `::`, `.` members, instantiation
/// types, named ports/params. Not this-file lexical (local decls, generate
/// / block identifiers, the left of `u0.leaf_wire`).
pub(crate) fn is_compilation_name(tp: SyntaxTokenWithParent<'_>) -> bool {
    if colon_colon_query(tp).is_some() {
        return true;
    }
    let SyntaxTokenWithParent { parent, tok } = tp;
    if SyntaxAncestors::start_from(parent)
        .find_map(ast::MemberAccessExpression::cast)
        .is_some_and(|access| access.name() == Some(tok))
    {
        return true;
    }
    if let Some(scoped) = SyntaxAncestors::start_from(parent).find_map(ast::ScopedName::cast)
        && scoped_uses_dot(scoped)
        && scoped_right_token(scoped) == Some(tok)
    {
        return true;
    }
    if SyntaxAncestors::start_from(parent)
        .find_map(ast::HierarchyInstantiation::cast)
        .is_some_and(|instantiation| instantiation.type_() == Some(tok))
    {
        return true;
    }
    if SyntaxAncestors::start_from(parent)
        .find_map(ast::CheckerInstantiation::cast)
        .is_some_and(|instantiation| rightmost_name_token(instantiation.type_()) == Some(tok))
    {
        return true;
    }
    if SyntaxAncestors::start_from(parent)
        .find_map(ast::PrimitiveInstantiation::cast)
        .is_some_and(|instantiation| instantiation.type_() == Some(tok))
    {
        return true;
    }
    if SyntaxAncestors::start_from(parent).any(|node| ast::NamedType::cast(node).is_some()) {
        return true;
    }
    if SyntaxAncestors::start_from(parent)
        .find_map(ast::NamedPortConnection::cast)
        .is_some_and(|connection| connection.name() == Some(tok))
    {
        return true;
    }
    if SyntaxAncestors::start_from(parent)
        .find_map(ast::NamedParamAssignment::cast)
        .is_some_and(|assignment| assignment.name() == Some(tok))
    {
        return true;
    }
    false
}

/// This-file lexical, or a paid-parse generated name (`HirFileId::Macro`).
/// Catalog identity in another file is not local.
pub(crate) fn hir_origin_is_local_or_generated(
    db: &dyn HirDefDb,
    current: FileId,
    origin: DefOrigin,
) -> bool {
    match origin.source(db).map(|source| source.file_id) {
        Some(HirFileId::Macro(_)) => true,
        Some(HirFileId::File(file)) => file == current,
        None => false,
    }
}

/// Dotted member / hierarchical right-hand name: `u0.leaf_wire`, `inst.m`.
pub(crate) fn dotted_member_query(tp: SyntaxTokenWithParent<'_>) -> Option<(String, String)> {
    let SyntaxTokenWithParent { parent, tok } = tp;
    if let Some(access) =
        SyntaxAncestors::start_from(parent).find_map(ast::MemberAccessExpression::cast)
        && access.name() == Some(tok)
    {
        let left = identifier_expr_name(access.left())?;
        return Some((left, tok.raw_text().to_string()));
    }
    let scoped = SyntaxAncestors::start_from(parent).find_map(ast::ScopedName::cast)?;
    if !scoped_uses_dot(scoped) {
        return None;
    }
    let right = scoped_right_token(scoped)?;
    if right != tok {
        return None;
    }
    let left = scoped_left_token(scoped)?;
    Some((left.tok.raw_text().to_string(), right.raw_text().to_string()))
}

fn identifier_expr_name(expr: ast::Expression<'_>) -> Option<String> {
    ast::IdentifierName::cast(expr.syntax())
        .and_then(|name| name.identifier())
        .map(|tok| tok.raw_text().to_string())
}

pub(crate) fn colon_colon_query(tp: SyntaxTokenWithParent<'_>) -> Option<(String, String)> {
    if let Some(item) =
        SyntaxAncestors::start_from(tp.parent).find_map(ast::PackageImportItem::cast)
    {
        let package = item.package()?;
        let package_name = package.raw_text().to_string();
        if item.package() == Some(tp.tok) {
            return Some((package_name, String::new()));
        }
        if item.item() == Some(tp.tok) {
            return Some((package_name, tp.tok.raw_text().to_string()));
        }
        return None;
    }
    let scoped = SyntaxAncestors::start_from(tp.parent).find_map(ast::ScopedName::cast)?;
    if scoped_uses_dot(scoped) {
        return None;
    }
    let left = scoped_left_token(scoped)?;
    let left_name = left.tok.raw_text().to_string();
    if left.tok == tp.tok {
        return Some((left_name, String::new()));
    }
    let right = scoped_right_token(scoped)?;
    if right == tp.tok {
        return Some((left_name, right.raw_text().to_string()));
    }
    None
}

fn resolve_instantiation_type_name(
    context: &hir_def::pathres::ResolutionContext,
    sema: &SemanticsImpl,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
    container: Option<OwnerId>,
) -> Option<DefinitionResolution> {
    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::PrimitiveInstantiation::cast)
        && instantiation.type_() == Some(tok)
    {
        return Some(
            nameres_ident(sema, file_id, tp, NameContext::Value, container)
                .map(DefinitionClass::Definition),
        );
    }

    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::CheckerInstantiation::cast)
        && rightmost_name_token(instantiation.type_()) == Some(tok)
    {
        return Some(
            nameres_ident(sema, file_id, tp, NameContext::Type, container)
                .map(DefinitionClass::Definition),
        );
    }

    if let Some(instantiation) =
        SyntaxAncestors::start_from(parent).find_map(ast::HierarchyInstantiation::cast)
        && instantiation.type_() == Some(tok)
    {
        // Paid-parse generated owners first (HirFileId::Macro). Then this-file
        // source CU owners and lexical nameres. Cross-file source hierarchy
        // is the compilation, not catalog OwnerId binding.
        if let Some(name) = hir_def::lower_ident_opt(Some(tok)) {
            let generated = Resolution::from_candidates(
                context
                    .locate_generated_hierarchy_targets(sema.db, &name)
                    .into_iter()
                    .filter_map(|owner| DefId::from_owner(sema.db, owner)),
            );
            if !generated.is_unresolved() {
                return Some(generated.map(DefinitionClass::Definition));
            }
            if let Some(file) = file_id.as_file() {
                let local_cu = Resolution::from_candidates(
                    hir_def::unit::cu_owners_named_in_file(sema.db, file, &name, |kind| {
                        kind.is_hierarchy_target()
                    })
                    .into_iter()
                    .filter_map(|owner| DefId::from_owner(sema.db, owner)),
                );
                if !local_cu.is_unresolved() {
                    return Some(local_cu.map(DefinitionClass::Definition));
                }
            }
        }
        return Some(
            nameres_ident(sema, file_id, tp, NameContext::Type, container)
                .or_else(|| {
                    Resolution::from_candidates(
                        nameres_ident(sema, file_id, tp, NameContext::Value, container)
                            .into_candidates()
                            .into_iter()
                            .filter(|def| def.kind(sema.db) == DefKind::Udp),
                    )
                })
                .map(DefinitionClass::Definition),
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

pub(crate) fn rightmost_name_token(name: ast::Name<'_>) -> Option<SyntaxToken<'_>> {
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

    use base_db::{change::Change, source_root::SourceRoot};
    use hir_def::symbol::DefKind;
    use hir_semantics::semantics::Semantics;
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

    fn host_with_profile_files(files: &[(&str, &str)]) -> (AnalysisHost, Vec<FileId>) {
        use base_db::{
            project::{CompilationProfile, CompilationProfileId, ProjectConfig},
            source_root::SourceRootId,
        };
        use triomphe::Arc;

        let mut file_set = FileSet::default();
        let mut change = Change::new();
        let mut ids = Vec::new();
        for (idx, (path, text)) in files.iter().enumerate() {
            let file_id = FileId::from_raw(idx as u32);
            file_set.insert(file_id, VfsPath::new_virtual_path((*path).to_owned()));
            change.add_changed_file(ChangedFile::create(file_id, *text));
            ids.push(file_id);
        }
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: Default::default(),
            }],
        )));
        let mut host = AnalysisHost::default();
        host.apply_change(change);
        (host, ids)
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
            let db = host.ctx();
            let sema = Semantics::<RootDb>::new_with_context(db.db, db.resolution());
            let parsed_file = sema.parse_file(file_id);
            let file = parsed_file.compilation_unit().unwrap();
            let tokens = file.syntax().token_at_offset(offset);
            let token = match pick {
                TokenPick::LeftBiased => tokens.left_biased(),
                TokenPick::GotoDefinition => {
                    tokens.pick_best_token(crate::token::navigation_precedence)
                }
            }
            .unwrap();
            let DefinitionClass::Definition(def) =
                DefinitionClass::resolve(&db, file_id.into(), token).unique().unwrap()
            else {
                panic!("expected plain definition for {name}");
            };

            let origins = def.origins(db.db);
            let (resolution, range) = match origins.first().cloned() {
                Some(origin) if origin.kind(db.db) == DefKind::NonAnsiPort => (
                    "NonAnsiPort",
                    origin.name_range(db.db).expect("non-ANSI port label should have a name range"),
                ),
                Some(origin) if origin.kind(db.db) == DefKind::Port => (
                    "AnsiPort",
                    origin.name_range(db.db).expect("ANSI port should have a name range"),
                ),
                other => panic!("unexpected definition for {name}: {other:?}"),
            };
            let range_start = usize::from(range.value.start());
            let range_end = usize::from(range.value.end());

            writeln!(&mut report, "{name}:").unwrap();
            writeln!(&mut report, "  resolution: {resolution}").unwrap();
            writeln!(&mut report, "  same_file: {}", range.file_id.as_file() == Some(file_id))
                .unwrap();
            writeln!(&mut report, "  name_range: {:?}", range.value).unwrap();
            writeln!(&mut report, "  name_text: {:?}", &text[range_start..range_end]).unwrap();
            writeln!(&mut report, "  starts_before_caret: {}", range.value.start() < offset)
                .unwrap();
        }

        insta::assert_snapshot!(report);
    }

    #[test]
    fn goto_hierarchical_path_leaf_is_the_compilation() {
        let text = r#"
module leaf;
  wire leaf_wire;
endmodule

module top;
  leaf u0();
  wire sink;
  initial sink = u0.leaf_/*caret*/wire;
"#;
        let offset = TextSize::from(text.find("/*caret*/").unwrap() as u32);
        let def_at = TextSize::from(text.find("leaf_wire;").unwrap() as u32);
        let text = text.replace("/*caret*/", "");
        let (host, file_id) = host_with_file(&text);
        let nav = host
            .make_analysis()
            .goto_definition(crate::FilePosition { file_id, offset })
            .unwrap()
            .expect("u0.leaf_wire is lookup on the compilation");
        assert_eq!(nav.info.len(), 1, "compilation binds; Ambiguous is not a jump: {nav:?}");
        assert!(
            nav.info
                .iter()
                .any(|target| target.focus_range.map(|range| range.start()) == Some(def_at)),
            "hierarchical member must land on leaf_wire: {nav:?}"
        );
    }

    #[test]
    fn instantiation_type_resolution_does_not_use_catalog_owner_binding() {
        let src = include_str!("definitions.rs");
        let locate = ["locate_hierarchy", "targets"].join("_");
        assert!(!src.contains(&locate), "instantiation type resolve must not call {locate}");
    }

    /// P5.2: cross-file hierarchy is the compilation. HIR must not bind the
    /// instantiation type through catalog → OwnerId.
    #[test]
    fn hir_does_not_bind_a_cross_file_instantiation_type() {
        let child = "module child;\nendmodule\n";
        let top = "module top;\n  chi/*caret*/ld u();\nendmodule\n";
        let offset = TextSize::from(top.find("/*caret*/").unwrap() as u32);
        let top = top.replace("/*caret*/", "");
        let (host, files) = host_with_profile_files(&[("/child.sv", child), ("/top.sv", &top)]);
        let child_id = files[0];
        let top_id = files[1];
        let db = host.ctx();
        let sema = Semantics::<RootDb>::new_with_context(db.db, db.resolution());
        let parsed = sema.parse_file(top_id);
        let token = parsed
            .compilation_unit()
            .unwrap()
            .syntax()
            .token_at_offset(offset)
            .pick_best_token(crate::token::navigation_precedence)
            .unwrap();

        let resolution = DefinitionClass::resolve(&db, top_id.into(), token);
        assert!(
            resolution.is_unresolved(),
            "HIR must not bind cross-file hierarchy: {resolution:?}"
        );

        let nav = host
            .make_analysis()
            .goto_definition(crate::FilePosition { file_id: top_id, offset })
            .unwrap()
            .expect("compilation must still find child");
        assert!(
            nav.info.iter().any(|target| {
                target.file_id == child_id && target.name.as_deref() == Some("child")
            }),
            "goto must land on child.sv via compilation: {nav:?}"
        );
    }

    /// P5.2: named ports are compilation names. HIR must not bind them through
    /// pathres catalog hierarchy location → other-file OwnerId.
    #[test]
    fn hir_does_not_bind_a_cross_file_named_port() {
        let child = "module child(input wire clk);\nendmodule\n";
        let top = "module top;\n  logic clk;\n  child u(.cl/*caret*/k(clk));\nendmodule\n";
        let offset = TextSize::from(top.find("/*caret*/").unwrap() as u32);
        let top = top.replace("/*caret*/", "");
        let (host, files) = host_with_profile_files(&[("/child.sv", child), ("/top.sv", &top)]);
        let child_id = files[0];
        let top_id = files[1];
        let db = host.ctx();
        let sema = Semantics::<RootDb>::new_with_context(db.db, db.resolution());
        let parsed = sema.parse_file(top_id);
        let token = parsed
            .compilation_unit()
            .unwrap()
            .syntax()
            .token_at_offset(offset)
            .pick_best_token(crate::token::navigation_precedence)
            .unwrap();

        let resolution = DefinitionClass::resolve(&db, top_id.into(), token);
        assert!(
            resolution.is_unresolved(),
            "HIR must not bind a cross-file named port: {resolution:?}"
        );

        let nav = host
            .make_analysis()
            .goto_definition(crate::FilePosition { file_id: top_id, offset })
            .unwrap()
            .expect("compilation must still find clk");
        assert!(
            nav.info.iter().any(|target| {
                target.file_id == child_id && target.name.as_deref() == Some("clk")
            }),
            "goto must land on child.sv clk via compilation: {nav:?}"
        );
    }

    #[test]
    fn unresolved_member_does_not_fall_back_to_lexical_name() {
        let text = r#"
module child;
endmodule

module top;
  child c();
  wire missing;
  wire sink = c.mi/*caret*/ssing;
endmodule
"#;
        let offset = TextSize::from(text.find("/*caret*/").unwrap() as u32);
        let text = text.replace("/*caret*/", "");
        let (host, file_id) = host_with_file(&text);
        let sema = Semantics::<RootDb>::new_with_context(host.ctx().db, host.ctx().resolution());
        let parsed = sema.parse_file(file_id);
        let token = parsed
            .compilation_unit()
            .unwrap()
            .syntax()
            .token_at_offset(offset)
            .pick_best_token(crate::token::navigation_precedence)
            .unwrap();

        assert_eq!(
            DefinitionClass::resolve(&host.ctx(), file_id.into(), token),
            Resolution::Unresolved
        );
    }

    #[test]
    fn named_parameter_resolution_preserves_ambiguity() {
        let text = r#"
module target #(parameter A = 1, parameter A = 2);
endmodule

module top;
  target #(.A/*caret*/(3)) u();
endmodule
"#;
        let offset = TextSize::from(text.find("/*caret*/").unwrap() as u32);
        let text = text.replace("/*caret*/", "");
        let (host, file_id) = host_with_file(&text);
        let db = host.ctx();
        let sema = Semantics::<RootDb>::new_with_context(db.db, db.resolution());
        let parsed = sema.parse_file(file_id);
        let token = parsed
            .compilation_unit()
            .unwrap()
            .syntax()
            .token_at_offset(offset)
            .pick_best_token(crate::token::navigation_precedence)
            .unwrap();

        let Resolution::Ambiguous(candidates) =
            DefinitionClass::resolve(&db, file_id.into(), token)
        else {
            panic!("duplicate named parameters should remain ambiguous");
        };
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(
            |candidate| matches!(candidate, DefinitionClass::Definition(def) if def.kind(db.db) == DefKind::Param)
        ));
    }

    #[test]
    fn package_colon_colon_is_answered_by_slang_when_the_package_name_is_duplicate() {
        for (case, text) in [
            (
                "scoped member",
                r#"
package p;
  int only_left;
endpackage

package p;
endpackage

module top;
  int x = p::only_/*caret*/left;
endmodule
"#,
            ),
            (
                "explicit import",
                r#"
package p;
  int only_left;
endpackage

package p;
endpackage

module top;
  import p::only_/*caret*/left;
endmodule
"#,
            ),
        ] {
            let offset = TextSize::from(text.find("/*caret*/").unwrap() as u32);
            let def_at = TextSize::from(text.find("only_left;").unwrap() as u32);
            let text = text.replace("/*caret*/", "");
            let (host, file_id) = host_with_file(&text);
            let nav = host
                .make_analysis()
                .goto_definition(crate::FilePosition { file_id, offset })
                .unwrap()
                .unwrap_or_else(|| panic!("{case}: slang must pick a p::only_left"));
            assert!(
                nav.info
                    .iter()
                    .any(|target| target.focus_range.map(|range| range.start()) == Some(def_at)),
                "{case} should land on only_left: {nav:?}"
            );
        }
    }

    #[test]
    fn udp_instantiation_type_resolves_in_value_namespace() {
        let text = r#"
primitive udp_and(out, in);
  output out;
  input in;
  table
    0 : 0;
  endtable
endprimitive

module top;
  wire sig;
  udp_/*caret*/and u(sig, sig);
endmodule
"#;
        let offset = TextSize::from(text.find("/*caret*/").unwrap() as u32);
        let text = text.replace("/*caret*/", "");
        let (host, file_id) = host_with_file(&text);
        let db = host.ctx();
        let sema = Semantics::<RootDb>::new_with_context(db.db, db.resolution());
        let parsed = sema.parse_file(file_id);
        let token = parsed
            .compilation_unit()
            .unwrap()
            .syntax()
            .token_at_offset(offset)
            .pick_best_token(crate::token::navigation_precedence)
            .unwrap();

        let resolution = DefinitionClass::resolve(&db, file_id.into(), token);
        let Some(DefinitionClass::Definition(def)) = resolution.unique() else {
            panic!("UDP type should resolve uniquely, got {resolution:?}");
        };
        assert_eq!(def.kind(db.db), DefKind::Udp);
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
        let db = host.ctx();
        let sema = Semantics::<RootDb>::new_with_context(db.db, db.resolution());
        let parsed_file = sema.parse_file(file_id);
        let file = parsed_file.compilation_unit().unwrap();
        let token = file
            .syntax()
            .token_at_offset(offset)
            .pick_best_token(crate::token::navigation_precedence)
            .unwrap();

        let resolution = DefinitionClass::resolve(&db, file_id.into(), token);
        let Resolution::Ambiguous(candidates) = resolution else {
            panic!("duplicate declarations should produce an ambiguous definition resolution");
        };
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|candidate| {
            matches!(candidate, DefinitionClass::Definition(def) if def.origins(db.db).len() == 1)
        }));
    }
}
