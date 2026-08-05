use base_db::source_db::SourceDb;
use hir_def::{container::InFile, def_id::DefId, symbol::DefOrigin};
use hir_semantics::semantics::Semantics;
use nohash_hasher::IntMap;
use preproc_expand::{
    file::HirFileId,
    macro_file::{macro_file_call_site, macro_files_at_offset},
};
use smol_str::SmolStr;
use syntax::{TokenKind, token::TokenKindExt};
use thiserror::Error;
use utils::{line_index::TextRange, text_edit::TextEdit, uniq_vec::UniqVec};
use vfs::FileId;

use crate::{
    FilePosition, ScopeVisibility,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    references::{
        ReferencesConfig,
        search::{ReferenceToken, ReferencesCtx, SearchScope},
    },
    semantic_index::{ConnSide, ReferenceContext},
    semantic_target::{SemanticTarget, TargetIntent, resolve_semantic_target},
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
        def: &DefId,
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
    #[error("Cannot rename a macro-generated definition")]
    MacroDefinitionNotEditable,
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
    position @ FilePosition { file_id, .. }: FilePosition,
    config: RenameConfig,
) -> RenameResult<TextRange> {
    let sema = Semantics::new(db);
    let target = resolve_rename_target(&sema, position)?;
    let _ = config.references_config(db, &target.selected_def, file_id)?;
    Ok(target.range)
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
    let mut rename_targets = UniqVec::<(), DefOrigin>::default();
    for target in &targets {
        rename_targets.push(target.def.origins(db), ());
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
    let targets: Vec<DefId> = if recursive {
        recursive_rename_targets(db, &sema, position.file_id, &config, resolved.targets)?
            .into_iter()
            .map(|target| target.def)
            .collect()
    } else {
        vec![resolved.selected_def]
    };

    let new_name = SmolStr::new(new_name);
    let mut target_index = UniqVec::<(), DefOrigin>::default();
    for target in &targets {
        target_index.push(target.origins(db), ());
    }
    let mut conflicts = UniqVec::<DefId, DefOrigin>::default();
    for collision in targets.iter().flat_map(|target| target.origins(db)).flat_map(|origin| {
        sema.resolve_name(origin.container_id(db), &new_name, origin.kind(db).name_context())
            .into_candidates()
    }) {
        if collision.origins(db).iter().any(|origin| target_index.contains(origin)) {
            continue;
        }
        conflicts.push(collision.origins(db), collision);
    }

    Ok(RenameCollisionInfo { conflicts: conflicts.len() })
}

struct ResolvedRenameTarget {
    range: TextRange,
    selected_def: DefId,
    targets: Vec<DefId>,
}

type ReferenceSearchResult = IntMap<FileId, Vec<ReferenceToken>>;

struct RecursiveRenameTarget {
    def: DefId,
    refs: ReferenceSearchResult,
}

/// The edit a single reference contributes to a rename, decided from the
/// index-recorded connection context and the file text alone: rename no
/// longer parses or re-resolves anything per reference.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReferenceEdit {
    /// Replace the reference range with the new name.
    Replace(TextRange),
    /// Replace a range with an arbitrary string (collapse or shorthand
    /// expansion).
    ReplaceWith(TextRange, String),
    /// Skip this reference: the paired side of the connection already
    /// produced the edit.
    Skip,
}

fn resolve_rename_target(
    sema: &Semantics<'_, RootDb>,
    FilePosition { file_id, offset }: FilePosition,
) -> RenameResult<ResolvedRenameTarget> {
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let target = resolve_semantic_target(
        sema.db,
        file_id,
        offset,
        parsed_file.root(),
        rename_token_precedence,
    );
    let SemanticTarget::Source(target) =
        target.unique_for_intent(TargetIntent::Rename).ok_or(RenameError::NoRefFound)?
    else {
        return Err(RenameError::NoRefFound);
    };
    let (range, tokens) = target.into_parts();
    let mut selected_def = None;
    let mut targets = UniqVec::<DefId, DefOrigin>::default();

    for token in tokens {
        let token_selected = match DefinitionClass::resolve(sema.db, hir_file_id, token)
            .unique()
            .ok_or(RenameError::NoDefFound)?
        {
            DefinitionClass::Definition(def) => {
                targets.push(def.origins(sema.db), def.clone());
                def
            }
            DefinitionClass::PortConnShorthand { port, local } => {
                targets.push(local.origins(sema.db), local.clone());
                targets.push(port.origins(sema.db), port.clone());
                local
            }
        };

        match &selected_def {
            Some(selected_def) if selected_def != &token_selected => {
                return Err(RenameError::NoDefFound);
            }
            Some(_) => {}
            None => selected_def = Some(token_selected),
        }
    }

    let selected_def = selected_def.ok_or(RenameError::NoDefFound)?;
    let targets = targets.into_vec();
    if targets
        .iter()
        .flat_map(|def| def.origins(sema.db))
        .any(|origin| origin_is_macro_generated(sema.db, origin))
    {
        return Err(RenameError::MacroDefinitionNotEditable);
    }
    Ok(ResolvedRenameTarget { range, selected_def, targets })
}

fn rename_definition(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    request_file_id: FileId,
    config: &RenameConfig,
    def: &DefId,
    new_name: &str,
    rename_targets: Option<&UniqVec<(), DefOrigin>>,
) -> RenameResult<SourceChange> {
    let refs = references_for_definition(db, sema, request_file_id, config, def)?;
    rename_definition_with_refs(db, sema, def, new_name, rename_targets, &refs)
}

fn references_for_definition(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    request_file_id: FileId,
    config: &RenameConfig,
    def: &DefId,
) -> RenameResult<ReferenceSearchResult> {
    let refs_config = config.references_config(db, def, request_file_id)?;
    Ok(ReferencesCtx::new(sema, def, refs_config).search())
}

fn rename_definition_with_refs(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    def: &DefId,
    new_name: &str,
    rename_targets: Option<&UniqVec<(), DefOrigin>>,
    refs: &ReferenceSearchResult,
) -> RenameResult<SourceChange> {
    let old_name = def
        .origins(db)
        .into_iter()
        .find_map(|origin| origin.name(db))
        .ok_or(RenameError::NoRefFound)?;
    let mut source_changes = SourceChange::default();
    refs.iter()
        .map(|(&file_id, toks)| {
            let text = sema.db.file_text(file_id);
            let mut text_edit = TextEdit::builder();
            for token_ref in toks {
                match reference_edit(
                    db,
                    token_ref.context(),
                    token_ref.range(),
                    &text,
                    &old_name,
                    new_name,
                    rename_targets,
                ) {
                    ReferenceEdit::Replace(range) => text_edit.replace(range, new_name.to_owned()),
                    ReferenceEdit::ReplaceWith(range, replacement) => {
                        text_edit.replace(range, replacement)
                    }
                    ReferenceEdit::Skip => {}
                }
            }
            (file_id, text_edit.finish())
        })
        .try_for_each(|(file_id, edit)| {
            source_changes
                .insert_text_edit(file_id, edit)
                .map_err(|_| RenameError::OverlappingEdits)
        })?;

    for def in def.origins(db) {
        let Some(InFile { value: focus_range, file_id }) = def.name_range(db) else {
            continue;
        };
        let Some(file_id) = file_id.as_file() else {
            continue;
        };

        source_changes
            .insert_text_edit(file_id, TextEdit::replace(focus_range, new_name.to_owned()))
            .map_err(|_| RenameError::OverlappingEdits)?;
    }

    Ok(source_changes)
}

/// Decides the edit for one reference from its index-recorded context and
/// the file text. `rename_targets` is `Some` for recursive renames: the set
/// of every definition being renamed, used to collapse same-name
/// connections (`.a(a)` becomes `.new`) instead of renaming both sides
/// independently.
fn reference_edit(
    db: &RootDb,
    context: &ReferenceContext,
    range: TextRange,
    text: &str,
    old_name: &str,
    new_name: &str,
    rename_targets: Option<&UniqVec<(), DefOrigin>>,
) -> ReferenceEdit {
    let paired_is_target = rename_targets.is_some_and(|targets| {
        context
            .paired()
            .is_some_and(|paired| paired.origins(db).iter().any(|origin| targets.contains(origin)))
    });
    match context {
        ReferenceContext::Plain => ReferenceEdit::Replace(range),
        ReferenceContext::ConnData { name_range, collapse_range, .. } => {
            if paired_is_target {
                // The name side already collapsed the connection.
                return ReferenceEdit::Skip;
            }
            // `.new(data) => .new`: the port name already equals the new name.
            if collapse_range.is_some_and(|_| range_text(text, *name_range) == new_name) {
                return ReferenceEdit::ReplaceWith(
                    collapse_range.expect("checked above"),
                    new_name.to_owned(),
                );
            }
            ReferenceEdit::Replace(range)
        }
        ReferenceContext::ConnName { ident_range, collapse_range, shorthand, side, .. } => {
            if *shorthand {
                if paired_is_target {
                    // Both sides are renamed: the port side rewrites the
                    // shorthand (`.a => .new`), the local side leaves it.
                    return match side {
                        ConnSide::Port => ReferenceEdit::Replace(range),
                        ConnSide::Local => ReferenceEdit::Skip,
                    };
                }
                return match side {
                    // `.old => .old(new)`: the local side is renamed.
                    ConnSide::Local => {
                        ReferenceEdit::ReplaceWith(range, format!("{old_name}({new_name})"))
                    }
                    // `.old => .new(old)`: the port side is renamed.
                    ConnSide::Port => {
                        ReferenceEdit::ReplaceWith(range, format!("{new_name}({old_name})"))
                    }
                };
            }
            // `.a(new) => .new`: the data already equals the new name.
            if ident_range.is_some_and(|ident| range_text(text, ident) == new_name)
                && let Some(collapse) = collapse_range
            {
                return ReferenceEdit::ReplaceWith(*collapse, new_name.to_owned());
            }
            // Same-name connection with both sides renamed: collapse the
            // whole `.a(a)` into `.new`.
            if paired_is_target && let Some(collapse) = collapse_range {
                return ReferenceEdit::ReplaceWith(*collapse, new_name.to_owned());
            }
            ReferenceEdit::Replace(range)
        }
    }
}

fn range_text(text: &str, range: TextRange) -> &str {
    &text[usize::from(range.start())..usize::from(range.end())]
}

fn recursive_rename_targets(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: FileId,
    config: &RenameConfig,
    initial_targets: Vec<DefId>,
) -> RenameResult<Vec<RecursiveRenameTarget>> {
    let mut targets = UniqVec::<DefId, DefOrigin>::default();
    for target in initial_targets {
        targets.push(target.origins(db), target);
    }
    let mut resolved_targets = Vec::new();
    let mut idx = 0;
    while idx < targets.len() {
        let current = targets.get(idx).clone();
        idx += 1;

        let refs = references_for_definition(db, sema, file_id, config, &current)?;
        // Same-name connections connect their paired definition: follow them
        // to close the recursive rename set.
        for toks in refs.values() {
            for token_ref in toks {
                if let Some(paired) = token_ref.context().paired() {
                    targets.push(paired.origins(db), paired.clone());
                }
            }
        }
        resolved_targets.push(RecursiveRenameTarget { def: current, refs });
    }

    Ok(resolved_targets)
}

fn origin_is_macro_generated(db: &RootDb, origin: DefOrigin) -> bool {
    if matches!(origin.container_id(db).file_id(db), HirFileId::Macro(_)) {
        return true;
    }
    let Some(InFile { file_id: HirFileId::File(file_id), value: range }) = origin.name_range(db)
    else {
        return false;
    };

    macro_files_at_offset(db, file_id, range.start()).into_iter().any(|macro_file| {
        macro_file_call_site(db, macro_file).is_some_and(|call_site| {
            call_site.call_file_id == file_id && call_site.call_range == range
        })
    })
}

fn origins_are_editable(db: &RootDb, def: &DefId, file_id: FileId) -> bool {
    def.origins(db).into_iter().all(|origin| {
        matches!(
            origin.name_range(db),
            Some(InFile { file_id: origin_file_id, .. }) if origin_file_id.as_file() == Some(file_id)
        )
    })
}

fn rename_token_precedence(kind: TokenKind) -> usize {
    usize::from(kind.name_like())
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_root::SourceRoot};
    use utils::text_edit::TextSize;
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;

    fn db_with_text(text: &str) -> (RootDb, FileId) {
        let file_id = FileId::from_raw(0);
        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/test.sv".to_owned()));

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.add_changed_file(ChangedFile::create(file_id, text));

        let mut db = RootDb::new(None);
        db.apply_change(change);
        (db, file_id)
    }

    fn db_with_caret(text: &str) -> (RootDb, FileId, TextSize) {
        let marker = "/*caret*/";
        let offset = text.find(marker).expect("missing caret marker");
        let text = text.replace(marker, "");
        let (db, file_id) = db_with_text(&text);
        (db, file_id, TextSize::from(offset as u32))
    }

    fn apply_rename(text: &str, new_name: &str, recursive: bool) -> String {
        let (db, file_id, offset) = db_with_caret(text);
        let config = RenameConfig::workspace(ScopeVisibility::Public);
        let position = FilePosition { file_id, offset };
        let change = if recursive {
            expanded_rename(&db, position, config, new_name)
        } else {
            rename(&db, position, config, new_name)
        }
        .expect("rename should succeed");
        let edit = change.text_edits.get(&file_id).expect("edit in the test file");
        let mut result = db.file_text(file_id).to_string();
        edit.apply(&mut result);
        result
    }

    fn check_rename(text: &str, new_name: &str, expected: &str) {
        assert_eq!(apply_rename(text, new_name, false), expected);
    }

    fn check_expanded_rename(text: &str, new_name: &str, expected: &str) {
        assert_eq!(apply_rename(text, new_name, true), expected);
    }

    #[test]
    fn plain_rename_updates_declaration_and_uses() {
        check_rename(
            "module m;\n  logic /*caret*/a;\n  always_comb a = a + 1;\nendmodule\n",
            "b",
            "module m;\n  logic b;\n  always_comb b = b + 1;\nendmodule\n",
        );
    }

    #[test]
    fn same_name_connection_renames_port_side_only() {
        check_rename(
            "module child(input a);\nendmodule\nmodule top;\n  logic a;\n  child u(./*caret*/a(a));\nendmodule\n",
            "b",
            "module child(input b);\nendmodule\nmodule top;\n  logic a;\n  child u(.b(a));\nendmodule\n",
        );
    }

    #[test]
    fn same_name_connection_renames_local_side_only() {
        check_rename(
            "module child(input a);\nendmodule\nmodule top;\n  logic /*caret*/a;\n  child u(.a(a));\nendmodule\n",
            "b",
            "module child(input a);\nendmodule\nmodule top;\n  logic b;\n  child u(.a(b));\nendmodule\n",
        );
    }

    #[test]
    fn same_name_connection_collapses_in_expanded_rename() {
        check_expanded_rename(
            "module child(input /*caret*/a);\nendmodule\nmodule top;\n  logic a;\n  child u(.a(a));\nendmodule\n",
            "b",
            "module child(input b);\nendmodule\nmodule top;\n  logic b;\n  child u(.b);\nendmodule\n",
        );
    }

    #[test]
    fn connection_data_equal_to_new_name_collapses() {
        check_rename(
            "module child(input a);\nendmodule\nmodule top;\n  logic c;\n  child u(./*caret*/a(c));\nendmodule\n",
            "c",
            "module child(input c);\nendmodule\nmodule top;\n  logic c;\n  child u(.c);\nendmodule\n",
        );
    }

    #[test]
    fn non_same_name_connection_renames_port_side() {
        check_rename(
            "module child(input a);\nendmodule\nmodule top;\n  logic b;\n  child u(./*caret*/a(b));\nendmodule\n",
            "c",
            "module child(input c);\nendmodule\nmodule top;\n  logic b;\n  child u(.c(b));\nendmodule\n",
        );
    }

    #[test]
    fn shorthand_connection_renames_port_side() {
        check_rename(
            "module child(input /*caret*/a);\nendmodule\nmodule top;\n  logic a;\n  child u(.a);\nendmodule\n",
            "b",
            "module child(input b);\nendmodule\nmodule top;\n  logic a;\n  child u(.b(a));\nendmodule\n",
        );
    }

    #[test]
    fn shorthand_connection_renames_local_side_from_shorthand() {
        check_rename(
            "module child(input a);\nendmodule\nmodule top;\n  logic a;\n  child u(./*caret*/a);\nendmodule\n",
            "b",
            "module child(input a);\nendmodule\nmodule top;\n  logic b;\n  child u(.a(b));\nendmodule\n",
        );
    }

    #[test]
    fn shorthand_connection_renames_local_side() {
        check_rename(
            "module child(input a);\nendmodule\nmodule top;\n  logic /*caret*/a;\n  child u(.a);\nendmodule\n",
            "b",
            "module child(input a);\nendmodule\nmodule top;\n  logic b;\n  child u(.a(b));\nendmodule\n",
        );
    }

    #[test]
    fn shorthand_connection_collapses_in_expanded_rename() {
        check_expanded_rename(
            "module child(input /*caret*/a);\nendmodule\nmodule top;\n  logic a;\n  child u(.a);\nendmodule\n",
            "b",
            "module child(input b);\nendmodule\nmodule top;\n  logic b;\n  child u(.b);\nendmodule\n",
        );
    }

    #[test]
    fn expanded_rename_does_not_follow_non_same_name_connections() {
        check_expanded_rename(
            "module child(input /*caret*/a);\nendmodule\nmodule top;\n  logic b;\n  child u(.a(b));\nendmodule\n",
            "c",
            "module child(input c);\nendmodule\nmodule top;\n  logic b;\n  child u(.c(b));\nendmodule\n",
        );
    }
}
