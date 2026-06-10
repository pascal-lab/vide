use hir::{
    base_db::source_db::SourceDb, container::InFile, semantics::Semantics,
    source_resolver::PositionResolver,
};
use nohash_hasher::IntMap;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use source_model::{
    FilePosition as SourceFilePosition, SourcePurpose, SourceTarget as GraphSourceTarget,
    SourceTargetResolution as GraphSourceTargetResolution,
};
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent,
    ast::{self, AstNode, Expression, Name},
    has_text_range::{HasTextRange, HasTextRangeIn},
    match_ast,
    token::TokenKindExt,
};
use thiserror::Error;
use utils::{
    line_index::{TextRange, TextSize},
    text_edit::TextEdit,
    uniq_vec::UniqVec,
};
use vfs::FileId;

use crate::{
    FilePosition, ScopeVisibility,
    db::root_db::RootDb,
    definitions::{Definition, DefinitionClass, DefinitionOrigin},
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
    position @ FilePosition { file_id, offset }: FilePosition,
    config: RenameConfig,
) -> RenameResult<TextRange> {
    ensure_source_graph_rename_target(db, position)?;
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root().ok_or(RenameError::NoRefFound)?;
    let token = pick_token(root, offset)?;
    let text_range = token.text_range().ok_or(RenameError::NoRefFound)?;
    let def =
        match DefinitionClass::resolve(&sema, hir_file_id, token).ok_or(RenameError::NoDefFound)? {
            DefinitionClass::Definition(def) => def,
            DefinitionClass::PortConnShorthand { local, .. } => local,
            DefinitionClass::Ambiguous(_) => return Err(RenameError::NoDefFound),
        };
    let _ = config.references_config(db, &def, file_id)?;
    Ok(text_range)
}

pub(crate) fn rename(
    db: &RootDb,
    position @ FilePosition { file_id, .. }: FilePosition,
    config: RenameConfig,
    new_name: &str,
) -> RenameResult<SourceChange> {
    let sema = Semantics::new(db);
    let ResolvedRenameTarget { selected_def, .. } = resolve_rename_target(&sema, position)?;
    rename_definition(db, &sema, file_id, &config, &selected_def, new_name, None)
}

pub(crate) fn rename_expansion_info(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
) -> RenameResult<RecursiveRenameInfo> {
    let sema = Semantics::new(db);
    let resolved = resolve_rename_target(&sema, position)?;
    let targets = recursive_rename_targets(db, &sema, position.file_id, &config, resolved.targets)?;
    let additional_symbols = targets.len().saturating_sub(1);
    Ok(RecursiveRenameInfo { additional_symbols })
}

pub(crate) fn expanded_rename(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
    new_name: &str,
) -> RenameResult<SourceChange> {
    let sema = Semantics::new(db);
    let resolved = resolve_rename_target(&sema, position)?;
    let targets = recursive_rename_targets(db, &sema, position.file_id, &config, resolved.targets)?;
    let mut rename_targets = UniqVec::<(), DefinitionOrigin>::default();
    for target in &targets {
        rename_targets.push(target.def.origins(), ());
    }
    let mut source_changes = SourceChange::default();

    for target in &targets {
        let changes = rename_definition_with_refs(
            db,
            &sema,
            &target.def,
            new_name,
            Some(&rename_targets),
            &target.refs,
            &target.same_name_refs,
        )?;
        for (file_id, edit) in changes.text_edits {
            source_changes
                .insert_text_edit(file_id, edit)
                .map_err(|_| RenameError::OverlappingEdits)?;
        }
    }

    Ok(source_changes)
}

pub(crate) fn rename_conflict_info(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
    new_name: &str,
    recursive: bool,
) -> RenameResult<RenameCollisionInfo> {
    let sema = Semantics::new(db);
    let resolved = resolve_rename_target(&sema, position)?;
    let targets: Vec<Definition> = if recursive {
        recursive_rename_targets(db, &sema, position.file_id, &config, resolved.targets)?
            .into_iter()
            .map(|target| target.def)
            .collect()
    } else {
        vec![resolved.selected_def]
    };

    let new_name = SmolStr::new(new_name);
    let mut target_index = UniqVec::<(), DefinitionOrigin>::default();
    for target in &targets {
        target_index.push(target.origins(), ());
    }
    let mut conflicts = UniqVec::<Definition, DefinitionOrigin>::default();
    for collision in targets.iter().flat_map(|target| target.origins()).filter_map(|origin| {
        sema.resolve_name(origin.container_id(db), &new_name).map(Definition::from)
    }) {
        if collision.origins().iter().any(|origin| target_index.contains(origin)) {
            continue;
        }
        conflicts.push(collision.origins(), collision);
    }

    Ok(RenameCollisionInfo { conflicts: conflicts.len() })
}

struct ResolvedRenameTarget {
    selected_def: Definition,
    targets: Vec<Definition>,
}

type ReferenceSearchResult = IntMap<FileId, Vec<ReferenceToken>>;

struct RecursiveRenameTarget {
    def: Definition,
    refs: ReferenceSearchResult,
    same_name_refs: Vec<SameNameConnectionRef>,
}

fn resolve_rename_target(
    sema: &Semantics<'_, RootDb>,
    position @ FilePosition { file_id, offset }: FilePosition,
) -> RenameResult<ResolvedRenameTarget> {
    ensure_source_graph_rename_target(sema.db, position)?;
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root().ok_or(RenameError::NoRefFound)?;
    let token = pick_token(root, offset)?;
    let mut targets = UniqVec::<Definition, DefinitionOrigin>::default();
    let selected_def = match DefinitionClass::resolve(sema, file_id.into(), token)
        .ok_or(RenameError::NoDefFound)?
    {
        DefinitionClass::Definition(def) => {
            targets.push(def.origins(), def.clone());
            def
        }
        DefinitionClass::PortConnShorthand { port, local } => {
            targets.push(local.origins(), local.clone());
            targets.push(port.origins(), port);
            local
        }
        DefinitionClass::Ambiguous(_) => return Err(RenameError::NoDefFound),
    };
    Ok(ResolvedRenameTarget { selected_def, targets: targets.into_vec() })
}

fn ensure_source_graph_rename_target(db: &RootDb, position: FilePosition) -> RenameResult<()> {
    let target = PositionResolver::new(db).resolve_position(
        SourceFilePosition { file_id: position.file_id, offset: position.offset },
        SourcePurpose::Rename,
        None,
    );

    match target {
        GraphSourceTargetResolution::Resolved(
            GraphSourceTarget::MacroDefinition(_)
            | GraphSourceTarget::MacroReference(_)
            | GraphSourceTarget::MacroCall(_)
            | GraphSourceTarget::MacroParamDefinition(_)
            | GraphSourceTarget::MacroParamReference(_)
            | GraphSourceTarget::Include(_)
            | GraphSourceTarget::ExpansionToken(_),
        )
        | GraphSourceTargetResolution::Ambiguous(_)
        | GraphSourceTargetResolution::Blocked(_) => Err(RenameError::NoDefFound),
        GraphSourceTargetResolution::Resolved(
            GraphSourceTarget::HirSymbol(_)
            | GraphSourceTarget::HirReference(_)
            | GraphSourceTarget::SyntaxToken(_),
        )
        | GraphSourceTargetResolution::None => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SameNameConnection {
    port: Definition,
    local: Definition,
    collapse_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SameNameConnectionRef {
    file_id: FileId,
    range: TextRange,
    conn: SameNameConnection,
}

fn rename_definition(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    request_file_id: FileId,
    config: &RenameConfig,
    def: &Definition,
    new_name: &str,
    rename_targets: Option<&UniqVec<(), DefinitionOrigin>>,
) -> RenameResult<SourceChange> {
    let refs = references_for_definition(db, sema, request_file_id, config, def)?;
    rename_definition_with_refs(db, sema, def, new_name, rename_targets, &refs, &[])
}

fn references_for_definition(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    request_file_id: FileId,
    config: &RenameConfig,
    def: &Definition,
) -> RenameResult<ReferenceSearchResult> {
    let refs_config = config.references_config(db, def, request_file_id)?;
    Ok(ReferencesCtx::new(sema, def, refs_config).search())
}

fn rename_definition_with_refs(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    def: &Definition,
    new_name: &str,
    rename_targets: Option<&UniqVec<(), DefinitionOrigin>>,
    refs: &ReferenceSearchResult,
    same_name_refs: &[SameNameConnectionRef],
) -> RenameResult<SourceChange> {
    let old_name = def
        .origins()
        .into_iter()
        .find_map(|origin| origin.name(db))
        .ok_or(RenameError::NoRefFound)?;
    let mut source_changes = SourceChange::default();
    refs.iter()
        .map(|(&file_id, toks)| {
            edits_from_refs(
                sema,
                file_id,
                toks,
                def,
                &old_name,
                new_name,
                rename_targets,
                same_name_refs,
            )
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
    file_id: FileId,
    config: &RenameConfig,
    initial_targets: Vec<Definition>,
) -> RenameResult<Vec<RecursiveRenameTarget>> {
    let mut targets = UniqVec::<Definition, DefinitionOrigin>::default();
    for target in initial_targets {
        targets.push(target.origins(), target);
    }
    let mut resolved_targets = Vec::new();
    let mut idx = 0;
    while idx < targets.len() {
        let current = targets.get(idx).clone();
        idx += 1;

        let refs = references_for_definition(db, sema, file_id, config, &current)?;
        let same_name_refs = same_name_refs_collect(sema, &refs);
        for conn_ref in &same_name_refs {
            targets.push(conn_ref.conn.port.origins(), conn_ref.conn.port.clone());
            targets.push(conn_ref.conn.local.origins(), conn_ref.conn.local.clone());
        }
        resolved_targets.push(RecursiveRenameTarget { def: current, refs, same_name_refs });
    }

    Ok(resolved_targets)
}

fn same_name_refs_collect(
    sema: &Semantics<'_, RootDb>,
    refs_by_file: &ReferenceSearchResult,
) -> Vec<SameNameConnectionRef> {
    let mut conn_refs = Vec::new();

    for (&file_id, refs) in refs_by_file {
        let parsed_file = sema.parse_file(file_id);
        for token_ref in refs {
            let range = token_ref.range();
            let Some(token) = token_ref.to_token(parsed_file.syntax_tree()) else {
                continue;
            };
            if let Some(conn) = check_same_name_conn(sema, file_id.into(), token) {
                conn_refs.push(SameNameConnectionRef { file_id, range, conn });
            };
        }
    }

    conn_refs
}

fn check_same_name_conn(
    sema: &Semantics<'_, RootDb>,
    file_id: hir::file::HirFileId,
    token: SyntaxTokenWithParent<'_>,
) -> Option<SameNameConnection> {
    let conn =
        SyntaxAncestors::start_from(token.parent).find_map(ast::NamedPortConnection::cast)?;
    let name_token = conn.name()?;
    let name_range = name_token.text_range_in(conn.syntax())?;
    let token_range = token.text_range()?;
    let port_token = SyntaxTokenWithParent { parent: conn.syntax(), tok: name_token };
    let port_resolution = DefinitionClass::resolve(sema, file_id, port_token)?;

    let close_paren = match (conn.open_paren(), conn.close_paren()) {
        (None, None) => {
            if token_range != name_range {
                return None;
            }

            return match port_resolution {
                DefinitionClass::PortConnShorthand { port, local } => {
                    Some(SameNameConnection { port, local, collapse_range: token_range })
                }
                _ => None,
            };
        }
        (_, Some(close_paren)) => close_paren,
        _ => return None,
    };

    let port = match port_resolution {
        DefinitionClass::Definition(def) => def,
        DefinitionClass::PortConnShorthand { port, .. } => port,
        DefinitionClass::Ambiguous(_) => return None,
    };
    let port_name = name_token.value_text().to_string();
    let expr = conn.expr()?.as_simple_property_expr()?.expr().as_simple_sequence_expr()?.expr();
    let actual_token = match expr {
        Expression::Name(Name::IdentifierName(ident)) => ident.identifier()?,
        Expression::Name(Name::IdentifierSelectName(ident))
            if ident.selectors().children().next().is_none() =>
        {
            ident.identifier()?
        }
        _ => return None,
    };
    if actual_token.value_text().to_string() != port_name {
        return None;
    }
    let actual_token = SyntaxTokenWithParent { parent: expr.syntax(), tok: actual_token };

    let actual_range = actual_token.text_range()?;
    if token_range != name_range && token_range != actual_range {
        return None;
    }

    let collapse_end = close_paren.text_range_in(conn.syntax())?.end();
    Some(SameNameConnection {
        port,
        local: Definition::from(sema.nameres_ident(file_id, actual_token)?),
        collapse_range: TextRange::new(name_range.start(), collapse_end),
    })
}

fn origins_are_editable(db: &RootDb, def: &Definition, file_id: FileId) -> bool {
    def.origins().into_iter().all(|origin| {
        matches!(
            origin.name_range(db),
            Some(InFile { file_id: origin_file_id, .. }) if origin_file_id.file_id() == file_id
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn edits_from_refs(
    sema: &Semantics<'_, RootDb>,
    file_id: FileId,
    toks: &[ReferenceToken],
    def: &Definition,
    old_name: &str,
    new_name: &str,
    rename_targets: Option<&UniqVec<(), DefinitionOrigin>>,
    same_name_refs: &[SameNameConnectionRef],
) -> (FileId, TextEdit) {
    let mut text_edit = TextEdit::builder();
    let text = sema.db.file_text(file_id);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let def_origins = def.origins();
    let same_name_refs: FxHashMap<_, _> = same_name_refs
        .iter()
        .filter(|it| it.file_id == file_id)
        .map(|SameNameConnectionRef { range, conn, .. }| {
            let SameNameConnection { port, local, collapse_range } = conn;
            (*range, (port.origins(), local.origins(), *collapse_range))
        })
        .collect();

    for token_ref in toks {
        let range = token_ref.range();
        let Some(token) = token_ref.to_token(parsed_file.syntax_tree()) else {
            continue;
        };
        let SyntaxTokenWithParent { parent, tok } = token;

        if let Some(rename_targets) = rename_targets
            && let Some((ports, locals, collapse_range)) = same_name_refs.get(&range)
            && ports.iter().any(|origin| rename_targets.contains(origin))
            && locals.iter().any(|origin| rename_targets.contains(origin))
        {
            if def_origins.iter().any(|origin| ports.contains(origin)) {
                text_edit.replace(*collapse_range, new_name.to_owned());
            }
            continue;
        }

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
                    .filter(|n| n.value_text().to_string() == new_name) {
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
