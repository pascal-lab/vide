use hir::{
    base_db::source_db::SourceDb,
    container::{ContainerId, InBlock, InFile, InGenerateBlock, InModule, InSubroutine},
    db::HirDb,
    hir_def::lower_ident,
    semantics::{Semantics, pathres::PathResolution},
};
use itertools::Itertools;
use smol_str::SmolStr;
use syntax::{
    SyntaxAncestors, SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, WalkEvent,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
    match_ast,
    token::TokenKindExt,
};
use thiserror::Error;
use utils::{
    line_index::{TextRange, TextSize},
    text_edit::TextEdit,
};
use vfs::FileId;

use crate::{
    FilePosition, ScopeVisibility,
    db::root_db::RootDb,
    definitions::{Definition, DefinitionClass},
    references::{
        ReferencesConfig,
        search::{ReferenceToken, ReferencesCtx, SearchScope},
    },
    source_change::SourceChange,
};

pub type RenameResult<T> = Result<T, RenameError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameEditScope {
    Workspace,
    SingleFile,
}

#[derive(Debug, Clone)]
pub struct RenameConfig {
    scope_visibility: ScopeVisibility,
    edit_scope: RenameEditScope,
}

impl RenameConfig {
    pub fn workspace(scope_visibility: ScopeVisibility) -> Self {
        Self { scope_visibility, edit_scope: RenameEditScope::Workspace }
    }

    pub fn with_edit_scope(mut self, edit_scope: RenameEditScope) -> Self {
        self.edit_scope = edit_scope;
        self
    }

    fn references_config(
        &self,
        db: &RootDb,
        def: &Definition,
        file_id: FileId,
    ) -> RenameResult<ReferencesConfig> {
        let mut config = ReferencesConfig::new(self.scope_visibility.clone(), None);

        match self.edit_scope {
            RenameEditScope::Workspace => Ok(config),
            RenameEditScope::SingleFile => {
                let natural_scope = config.search_scope(db, def);
                if !natural_scope.is_within_file(file_id) || !origins_are_editable(db, def, file_id)
                {
                    return Err(RenameError::ProjectScopeRequired);
                }

                config.search_scope = Some(SearchScope::single_file(file_id));
                Ok(config)
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum RenameError {
    #[error("No references found at position")]
    NoRefFound,
    #[error("No definitions found for the token")]
    NoDefFound,
    #[error("Generated overlapping edits")]
    OverlappingEdits,
    #[error("Project configuration required for this rename")]
    ProjectScopeRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveRenameInfo {
    pub additional_symbols: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameCollisionInfo {
    pub conflicts: usize,
}

pub(crate) fn prepare_rename(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    config: RenameConfig,
) -> RenameResult<TextRange> {
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root().ok_or(RenameError::NoRefFound)?;
    let token = pick_token(root, offset)?;
    let text_range = token.text_range().ok_or(RenameError::NoRefFound)?;
    let def = resolve_rename_definition(&sema, hir_file_id, token)?;
    let _ = config.references_config(db, &def, file_id)?;
    Ok(text_range)
}

pub(crate) fn rename(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
    new_name: &str,
) -> RenameResult<SourceChange> {
    let sema = Semantics::new(db);
    let def = rename_definition_at_position(&sema, position)?;
    rename_definition(db, &sema, position.file_id, config, &def, new_name, &[])
}

pub(crate) fn recursive_rename_info(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
) -> RenameResult<RecursiveRenameInfo> {
    let sema = Semantics::new(db);
    let selected_def = rename_definition_at_position(&sema, position)?;
    let targets = recursive_rename_targets(db, &sema, position, config)?;
    let additional_symbols = targets.iter().filter(|target| **target != selected_def).count();
    Ok(RecursiveRenameInfo { additional_symbols })
}

pub(crate) fn recursive_rename(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
    new_name: &str,
) -> RenameResult<SourceChange> {
    let sema = Semantics::new(db);
    let targets = recursive_rename_targets(db, &sema, position, config.clone())?;
    let collapsed_connections = collapsed_connections_for_targets(
        db,
        &sema,
        position.file_id,
        config.clone(),
        &targets,
        new_name,
    )?;
    let mut source_changes = SourceChange::default();

    for def in targets {
        merge_source_change(
            &mut source_changes,
            rename_definition(
                db,
                &sema,
                position.file_id,
                config.clone(),
                &def,
                new_name,
                &collapsed_connections,
            )?,
        )?;
    }

    for collapsed in collapsed_connections {
        let edit = TextEdit::replace(collapsed.range, collapsed.new_text);
        source_changes
            .insert_text_edit(collapsed.file_id, edit)
            .map_err(|_| RenameError::OverlappingEdits)?;
    }

    Ok(source_changes)
}

fn merge_source_change(target: &mut SourceChange, source: SourceChange) -> RenameResult<()> {
    for (file_id, edit) in source.text_edits {
        target.insert_text_edit(file_id, edit).map_err(|_| RenameError::OverlappingEdits)?;
    }

    Ok(())
}

pub(crate) fn rename_collision_info(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
    new_name: &str,
    recursive: bool,
) -> RenameResult<RenameCollisionInfo> {
    let sema = Semantics::new(db);
    let targets = if recursive {
        recursive_rename_targets(db, &sema, position, config)?
    } else {
        vec![rename_definition_at_position(&sema, position)?]
    };

    let new_name = SmolStr::new(new_name);
    let mut conflicts = Vec::new();
    for collision in targets
        .iter()
        .flat_map(|target| target.origins())
        .filter_map(|origin| exact_name_to_def(db, origin.container_id(db), &new_name))
        .filter(|collision| !contains_def(&targets, collision))
    {
        add_unique_def(&mut conflicts, collision);
    }

    Ok(RenameCollisionInfo { conflicts: conflicts.len() })
}

fn rename_definition_at_position(
    sema: &Semantics<'_, RootDb>,
    FilePosition { file_id, offset }: FilePosition,
) -> RenameResult<Definition> {
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root().ok_or(RenameError::NoRefFound)?;
    let token = pick_token(root, offset)?;
    resolve_rename_definition(sema, file_id.into(), token)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SameNamePortConnection {
    port: Definition,
    local: Definition,
    file_id: FileId,
    collapse_range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CollapsedPortConnection {
    file_id: FileId,
    range: TextRange,
    new_text: String,
}

fn rename_definition(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    request_file_id: FileId,
    config: RenameConfig,
    def: &Definition,
    new_name: &str,
    collapsed_connections: &[CollapsedPortConnection],
) -> RenameResult<SourceChange> {
    let refs_config = config.references_config(db, def, request_file_id)?;
    let old_name = def
        .origins()
        .into_iter()
        .find_map(|origin| origin.name(db))
        .ok_or(RenameError::NoRefFound)?;
    let mut source_changes = SourceChange::default();
    ReferencesCtx::new(sema, def, refs_config)
        .search()
        .into_iter()
        .map(|file_toks| {
            edits_from_refs(sema, file_toks, def, &old_name, new_name, collapsed_connections)
        })
        .try_for_each(|(file_id, edit)| {
            source_changes
                .insert_text_edit(file_id, edit)
                .map_err(|_| RenameError::OverlappingEdits)
        })?;

    for def in def.origins() {
        let Some(InFile { value: focus_range, file_id }) = def.name_range(db) else {
            continue;
        };

        source_changes
            .insert_text_edit(
                file_id.file_id(),
                TextEdit::replace(focus_range, new_name.to_owned()),
            )
            .map_err(|_| RenameError::OverlappingEdits)?;
    }

    Ok(source_changes)
}

fn recursive_rename_targets(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    position: FilePosition,
    config: RenameConfig,
) -> RenameResult<Vec<Definition>> {
    let mut targets = initial_recursive_targets(sema, position)?;

    let mut idx = 0;
    while idx < targets.len() {
        let current = targets[idx].clone();
        idx += 1;

        for conn in same_name_connections_from_references(
            db,
            sema,
            position.file_id,
            config.clone(),
            &current,
        )? {
            add_unique_def(&mut targets, conn.port);
            add_unique_def(&mut targets, conn.local);
        }
    }

    Ok(targets)
}

fn initial_recursive_targets(
    sema: &Semantics<'_, RootDb>,
    FilePosition { file_id, offset }: FilePosition,
) -> RenameResult<Vec<Definition>> {
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root().ok_or(RenameError::NoRefFound)?;
    let token = pick_token(root, offset)?;

    let mut targets = Vec::new();
    match DefinitionClass::resolve(sema, file_id.into(), token).ok_or(RenameError::NoDefFound)? {
        DefinitionClass::Definition(def) => add_unique_def(&mut targets, def),
        DefinitionClass::PortConnShorthand { port, local } => {
            add_unique_def(&mut targets, local);
            add_unique_def(&mut targets, port);
        }
        DefinitionClass::Ambiguous(_) => return Err(RenameError::NoDefFound),
    }

    Ok(targets)
}

fn collapsed_connections_for_targets(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    request_file_id: FileId,
    config: RenameConfig,
    targets: &[Definition],
    new_name: &str,
) -> RenameResult<Vec<CollapsedPortConnection>> {
    let mut collapsed = Vec::new();

    for target in targets {
        for conn in same_name_connections_from_references(
            db,
            sema,
            request_file_id,
            config.clone(),
            target,
        )? {
            if !contains_def(targets, &conn.port) || !contains_def(targets, &conn.local) {
                continue;
            }
            let Some(range) = conn.collapse_range else {
                continue;
            };
            collapsed.push(CollapsedPortConnection {
                file_id: conn.file_id,
                range,
                new_text: new_name.to_owned(),
            });
        }
    }

    Ok(collapsed.into_iter().unique_by(|conn| (conn.file_id, conn.range)).collect())
}

fn same_name_connections_from_references(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    request_file_id: FileId,
    config: RenameConfig,
    def: &Definition,
) -> RenameResult<Vec<SameNamePortConnection>> {
    let refs_config = config.references_config(db, def, request_file_id)?;
    let mut conns = Vec::new();

    for (ref_file_id, refs) in ReferencesCtx::new(sema, def, refs_config).search() {
        let parsed_file = sema.parse_file(ref_file_id);
        for token_ref in refs {
            let Some(token) = token_ref.to_token(parsed_file.syntax_tree()) else {
                continue;
            };
            let Some(conn) = same_name_port_connection_at_token(sema, ref_file_id.into(), token)
            else {
                continue;
            };
            conns.push(conn);
        }
    }

    Ok(conns)
}

fn same_name_port_connection_at_token(
    sema: &Semantics<'_, RootDb>,
    file_id: hir::file::HirFileId,
    token: SyntaxTokenWithParent<'_>,
) -> Option<SameNamePortConnection> {
    let conn =
        SyntaxAncestors::start_from(token.parent).find_map(ast::NamedPortConnection::cast)?;
    let name_token = conn.name()?;
    let port_name = lower_ident(Some(name_token))?;
    let token_range = token.text_range()?;

    if conn.open_paren().is_none() && conn.close_paren().is_none() {
        return shorthand_same_name_port_connection(sema, file_id, conn, name_token, token_range);
    }

    explicit_same_name_port_connection(sema, file_id, conn, name_token, &port_name, token_range)
}

fn shorthand_same_name_port_connection(
    sema: &Semantics<'_, RootDb>,
    file_id: hir::file::HirFileId,
    conn: ast::NamedPortConnection<'_>,
    name_token: syntax::SyntaxToken<'_>,
    token_range: TextRange,
) -> Option<SameNamePortConnection> {
    if name_token.text_range_in(conn.syntax())? != token_range {
        return None;
    }

    match DefinitionClass::resolve(
        sema,
        file_id,
        SyntaxTokenWithParent { parent: conn.syntax(), tok: name_token },
    )? {
        DefinitionClass::PortConnShorthand { port, local } => Some(SameNamePortConnection {
            port,
            local,
            file_id: file_id.file_id(),
            collapse_range: Some(token_range),
        }),
        _ => None,
    }
}

fn explicit_same_name_port_connection(
    sema: &Semantics<'_, RootDb>,
    file_id: hir::file::HirFileId,
    conn: ast::NamedPortConnection<'_>,
    name_token: syntax::SyntaxToken<'_>,
    port_name: &str,
    token_range: TextRange,
) -> Option<SameNamePortConnection> {
    let actual_token = simple_same_name_actual_token(sema, file_id, conn, port_name)?;
    let name_range = name_token.text_range_in(conn.syntax())?;
    let actual_range = actual_token.text_range()?;
    if token_range != name_range && token_range != actual_range {
        return None;
    }

    let port = resolve_named_connection_port(sema, file_id, conn, name_token)?;
    let local = Definition::from(sema.nameres_ident(file_id, actual_token)?);
    let close_paren = conn.close_paren()?;
    let collapse_end = close_paren.text_range_in(conn.syntax())?.end();

    Some(SameNamePortConnection {
        port,
        local,
        file_id: file_id.file_id(),
        collapse_range: Some(TextRange::new(name_range.start(), collapse_end)),
    })
}

fn resolve_named_connection_port(
    sema: &Semantics<'_, RootDb>,
    file_id: hir::file::HirFileId,
    conn: ast::NamedPortConnection<'_>,
    name_token: syntax::SyntaxToken<'_>,
) -> Option<Definition> {
    match DefinitionClass::resolve(
        sema,
        file_id,
        SyntaxTokenWithParent { parent: conn.syntax(), tok: name_token },
    )? {
        DefinitionClass::Definition(def) => Some(def),
        DefinitionClass::PortConnShorthand { port, .. } => Some(port),
        DefinitionClass::Ambiguous(_) => None,
    }
}

fn simple_same_name_actual_token<'a>(
    sema: &Semantics<'_, RootDb>,
    file_id: hir::file::HirFileId,
    conn: ast::NamedPortConnection<'a>,
    port_name: &str,
) -> Option<SyntaxTokenWithParent<'a>> {
    let expr = conn.expr()?;
    let range = expr.syntax().text_range()?;
    let text = sema.db.file_text(file_id.file_id());
    let compact = text[range].chars().filter(|ch| !ch.is_whitespace()).collect::<String>();
    if compact != port_name {
        return None;
    }

    let mut name_tokens = expr.syntax().elem_preorder().filter_map(|event| match event {
        WalkEvent::Enter(SyntaxElement::Token(token)) if token.kind().name_like() => Some(token),
        _ => None,
    });
    let token = name_tokens.next()?;
    if name_tokens.next().is_some() {
        return None;
    }
    if lower_ident(Some(token.tok)).as_deref() != Some(port_name) {
        return None;
    }

    Some(token)
}

fn add_unique_def(targets: &mut Vec<Definition>, def: Definition) {
    if !contains_def(targets, &def) {
        targets.push(def);
    }
}

fn contains_def(targets: &[Definition], def: &Definition) -> bool {
    targets.iter().any(|target| target == def)
}

fn exact_name_to_def(db: &RootDb, cont_id: ContainerId, ident: &SmolStr) -> Option<Definition> {
    let res = match cont_id {
        ContainerId::HirFileId(_) => db.unit_scope().get(ident).map(PathResolution::from),
        ContainerId::ModuleId(module_id) => db
            .module_scope(module_id)
            .get(ident)
            .map(|entry| PathResolution::from(InModule::new(module_id, entry))),
        ContainerId::GenerateBlockId(generate_block_id) => db
            .generate_block_scope(generate_block_id)
            .get(ident)
            .map(|entry| PathResolution::from(InGenerateBlock::new(generate_block_id, entry))),
        ContainerId::BlockId(block_id) => db
            .block_scope(block_id)
            .get(ident)
            .map(|entry| PathResolution::from(InBlock::new(block_id, entry))),
        ContainerId::SubroutineId(subroutine_id) => db
            .subroutine_scope(subroutine_id)
            .get(ident)
            .map(|entry| PathResolution::from(InSubroutine::new(subroutine_id, entry))),
    }?;

    Some(Definition::from(res))
}

fn resolve_rename_definition(
    sema: &Semantics<'_, RootDb>,
    hir_file_id: hir::file::HirFileId,
    token: SyntaxTokenWithParent<'_>,
) -> RenameResult<Definition> {
    match DefinitionClass::resolve(sema, hir_file_id, token).ok_or(RenameError::NoDefFound)? {
        DefinitionClass::Definition(def) => Ok(def),
        DefinitionClass::PortConnShorthand { local, .. } => Ok(local),
        DefinitionClass::Ambiguous(_) => Err(RenameError::NoDefFound),
    }
}

fn origins_are_editable(db: &RootDb, def: &Definition, file_id: FileId) -> bool {
    def.origins().into_iter().all(|origin| {
        matches!(
            origin.name_range(db),
            Some(InFile { file_id: origin_file_id, .. }) if origin_file_id.file_id() == file_id
        )
    })
}

fn edits_from_refs(
    sema: &Semantics<'_, RootDb>,
    (file_id, toks): (FileId, Vec<ReferenceToken>),
    def: &Definition,
    old_name: &str,
    new_name: &str,
    collapsed_connections: &[CollapsedPortConnection],
) -> (FileId, TextEdit) {
    let mut text_edit = TextEdit::builder();
    let text = sema.db.file_text(file_id);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);

    for token_ref in toks.into_iter() {
        let range = token_ref.range();
        if collapsed_connections.iter().any(|conn| {
            conn.file_id == file_id
                && conn.range.start() <= range.start()
                && range.end() <= conn.range.end()
        }) {
            continue;
        }

        let Some(token) = token_ref.to_token(parsed_file.syntax_tree()) else {
            continue;
        };
        let SyntaxTokenWithParent { parent, tok } = token;

        let conn_data_range = |it: ast::NamedPortConnection| it.expr()?.syntax().text_range();

        match_ast! { parent,
            ast::NamedPortConnection[it] if it.name() == Some(tok) => {
                // .[port](data)
                match (it.open_paren(), it.close_paren()) {
                    (Some(_), Some(cp)) if conn_data_range(it).is_some_and(|r| &text[r] == new_name) => {
                        // .port(new),  => .new,
                        if let Some(end) = cp.text_range_in(it.syntax()).map(|range| range.end()) {
                            text_edit.replace(TextRange::new(range.start(), end), new_name.to_owned());
                        } else {
                            text_edit.replace(range, new_name.to_owned());
                        }
                    }
                    (None, None) => {
                        if let Some(port_conn) = ast::PortConnection::cast(it.syntax()) {
                            if let Some(ref_container) = sema.resolve_port_connection(hir_file_id, port_conn)
                                && def
                                    .container_id(sema.db)
                                    .is_some_and(|id| id == ref_container.module_id.into())
                            {
                                // .old => .old(new)
                                text_edit.replace(range, format!("{old_name}({new_name})"));
                            } else {
                                // .old => .new(old)
                                text_edit.replace(range, format!("{new_name}({old_name})"));
                            }
                        } else {
                            text_edit.replace(range, new_name.to_owned());
                        }
                    }
                    _ => text_edit.replace(range, new_name.to_owned()),
                }
            },
            ast::IdentifierName => {
                if let Some(node) = SyntaxAncestors::start_from(parent).nth(3)
                && let Some(port_conn) = ast::NamedPortConnection::cast(node)
                && conn_data_range(port_conn).is_some_and(|r| r == range)
                && let Some(port_name) = port_conn
                    .name()
                    .filter(|n| lower_ident(Some(*n)).is_some_and(|name| name == new_name)) {
                    // .new(data) => .new
                    let Some(start) =
                        port_name.text_range_in(port_conn.syntax()).map(|range| range.start()) else {
                        text_edit.replace(range, new_name.to_owned());
                        continue;
                    };
                    let end = if let Some(cp) = port_conn.close_paren() {
                        cp.text_range_in(port_conn.syntax())
                            .map(|range| range.end())
                            .unwrap_or(range.end())
                    } else {
                        range.end()
                    };
                    text_edit.replace(TextRange::new(start, end), new_name.to_owned());
                } else {
                    text_edit.replace(range, new_name.to_owned());
                }
            },
            _ => text_edit.replace(range, new_name.to_owned()),
        }
    }

    (file_id, text_edit.finish())
}

fn pick_token(node: SyntaxNode, offset: TextSize) -> RenameResult<SyntaxTokenWithParent> {
    node.token_at_offset(offset)
        .pick_bext_token(|kind| kind.name_like().into())
        .ok_or(RenameError::NoRefFound)
}
