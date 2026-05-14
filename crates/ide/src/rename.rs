use base_db::source_db::SourceDb;
use hir::{container::InFile, hir_def::lower_ident, semantics::Semantics};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
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
        search::{ReferenceToken, ReferencesCtx},
    },
    source_change::SourceChange,
};

pub type RenameResult<T> = Result<T, RenameError>;

#[derive(Debug, Clone)]
pub struct RenameConfig {
    pub scope_visibility: ScopeVisibility,
}

#[derive(Error, Debug)]
pub enum RenameError {
    #[error("No references found at position")]
    NoRefFound,
    #[error("No definitions found for the token")]
    NoDefFound,
}

pub(crate) fn prepare_rename(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> RenameResult<TextRange> {
    let sema = Semantics::new(db);
    let root = sema.parse_root(file_id);
    let token = pick_token(root, offset)?;
    let text_range = token.text_range().ok_or(RenameError::NoRefFound)?;
    DefinitionClass::resolve(&sema, token).ok_or(RenameError::NoDefFound)?;
    Ok(text_range)
}

pub(crate) fn rename(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    RenameConfig { scope_visibility }: RenameConfig,
    new_name: &str,
) -> RenameResult<SourceChange> {
    let sema = Semantics::new(db);
    let root = sema.parse_root(file_id);
    let token = pick_token(root, offset)?;
    let def = match DefinitionClass::resolve(&sema, token).ok_or(RenameError::NoDefFound)? {
        DefinitionClass::Definition(def) => def,
        DefinitionClass::PortConnShorthand { local, .. } => local,
    };

    let old_name = lower_ident(Some(token.tok)).unwrap();
    let mut source_changes = SourceChange::default();
    ReferencesCtx::new(&sema, &def, ReferencesConfig::new(scope_visibility, None))
        .search()
        .into_iter()
        .map(|file_toks| edits_from_refs(&sema, file_toks, &def, &old_name, new_name))
        .for_each(|(file_id, edit)| source_changes.insert_text_edit(file_id, edit));

    def.origins().into_iter().for_each(|def| {
        let mut text_edit = TextEdit::builder();

        let InFile { value: focus_range, file_id } = def.name_range(db);
        text_edit.replace(focus_range, new_name.to_owned());

        source_changes.insert_text_edit(file_id.file_id(), text_edit.finish());
    });

    Ok(source_changes)
}

fn edits_from_refs(
    sema: &Semantics<'_, RootDb>,
    (file_id, toks): (FileId, Vec<ReferenceToken<'_>>),
    def: &Definition,
    old_name: &str,
    new_name: &str,
) -> (FileId, TextEdit) {
    let mut text_edit = TextEdit::builder();
    let text = sema.db.file_text(file_id);

    for ReferenceToken { token: SyntaxTokenWithParent { parent, tok } } in toks.into_iter() {
        let range = tok.text_range().unwrap();

        let conn_data_range = |it: ast::NamedPortConnection| it.expr()?.syntax().text_range();

        match_ast! { parent,
            ast::NamedPortConnection[it] if it.name() == Some(tok) => {
                // .[port](data)
                match (it.open_paren(), it.close_paren()) {
                    (Some(_), Some(cp)) if conn_data_range(it).is_some_and(|r| &text[r] == new_name) => {
                        // .port(new),  => .new,
                        let end = cp.text_range().unwrap().end();
                        text_edit.replace(TextRange::new(range.start(), end), new_name.to_owned());
                    }
                    (None, None) => {
                        let ref_container = sema.resolve_named_port_conn(ast::PortConnection::cast(it.syntax()).unwrap());
                        if def.container_id(sema.db) == ref_container.module_id.into() {
                            // .old => .old(new)
                            text_edit.replace(range, format!("{old_name}({new_name})"));
                        } else {
                            // .old => .new(old)
                            text_edit.replace(range, format!("{new_name}({old_name})"));
                        }
                    }
                    _ => text_edit.replace(range, new_name.to_owned()),
                }
            },
            ast::IdentifierName => {
                if let Some(node) = SyntaxAncestors::start_from(parent).nth(3)
                && let Some(port_conn) = ast::NamedPortConnection::cast(node)
                && conn_data_range(port_conn).is_some_and(|r| r == range)
                && let Some(port_name) = port_conn.name().filter(|n| lower_ident(Some(*n)).unwrap() == new_name) {
                    // .new(data) => .new
                    let start = port_name.text_range().unwrap().start();
                    let end = if let Some(cp) = port_conn.close_paren() {
                        cp.text_range().unwrap().end()
                    }else {
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
