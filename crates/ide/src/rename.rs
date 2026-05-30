use base_db::source_db::SourceDb;
use hir::{container::InFile, hir_def::lower_ident, semantics::Semantics};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent,
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
    ScopeVisibility,
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
    FilePosition { file_id, offset }: FilePosition,
    config: RenameConfig,
    new_name: &str,
) -> RenameResult<SourceChange> {
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root().ok_or(RenameError::NoRefFound)?;
    let token = pick_token(root, offset)?;
    let def = resolve_rename_definition(&sema, hir_file_id, token)?;
    let refs_config = config.references_config(db, &def, file_id)?;

    let old_name = lower_ident(Some(token.tok)).ok_or(RenameError::NoRefFound)?;
    let mut source_changes = SourceChange::default();
    ReferencesCtx::new(&sema, &def, refs_config)
        .search()
        .into_iter()
        .map(|file_toks| edits_from_refs(&sema, file_toks, &def, &old_name, new_name))
        .try_for_each(|(file_id, edit)| {
            source_changes
                .insert_text_edit(file_id, edit)
                .map_err(|_| RenameError::OverlappingEdits)
        })?;

    for def in def.origins() {
        let mut text_edit = TextEdit::builder();

        let Some(InFile { value: focus_range, file_id }) = def.name_range(db) else {
            continue;
        };
        text_edit.replace(focus_range, new_name.to_owned());

        source_changes
            .insert_text_edit(file_id.file_id(), text_edit.finish())
            .map_err(|_| RenameError::OverlappingEdits)?;
    }

    Ok(source_changes)
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
) -> (FileId, TextEdit) {
    let mut text_edit = TextEdit::builder();
    let text = sema.db.file_text(file_id);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);

    for token_ref in toks.into_iter() {
        let range = token_ref.range();
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
