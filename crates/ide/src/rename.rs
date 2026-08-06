use base_db::source_db::SourceDb;
use hir_def::{container::InFile, def_id::DefId, symbol::DefOrigin};
use hir_semantics::semantics::Semantics;
use nohash_hasher::IntMap;
use preproc_expand::{
    file::HirFileId,
    macro_file::{macro_file_call_site, macro_files_at_offset},
    preproc::{
        MacroDefinition, MacroParamDefinition, MacroReference, PreprocError,
        macro_param_references, macro_references,
    },
};
use smol_str::SmolStr;
use syntax::{TokenKind, token::TokenKindExt};
use thiserror::Error;
use utils::{line_index::TextRange, text_edit::TextEdit, uniq_vec::UniqVec};
use vfs::FileId;

use crate::{
    FilePosition, ScopeVisibility,
    db::{root_db::RootDb, workspace_symbol_index_db::WorkspaceSymbolIndexDb},
    definitions::DefinitionClass,
    references::{
        ReferencesConfig,
        search::{ReferenceToken, ReferencesCtx, SearchScope, search_references},
    },
    semantic_index::{ConnSide, ReferenceContext},
    semantic_target::{
        PreprocMacroTarget, SemanticTarget, SourceTarget, TargetIntent, resolve_semantic_target,
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
        def: &DefId,
        file_id: FileId,
    ) -> RenameResult<ReferencesConfig> {
        let mut config = ReferencesConfig::new(self.scope_visibility, None);

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
    #[error("Macro rename failed: {0:?}")]
    MacroRenameFailed(PreprocError),
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
    match &target {
        RenameTarget::Hdl(target) => {
            let _ = config.references_config(db, &target.selected_def, file_id)?;
        }
        RenameTarget::Macro(_) => {}
    }
    Ok(target.range())
}

pub(crate) fn rename(
    db: &RootDb,
    position @ FilePosition { file_id, .. }: FilePosition,
    config: RenameConfig,
    new_name: &str,
) -> RenameResult<SourceChange> {
    let sema = Semantics::new(db);
    match resolve_rename_target(&sema, position)? {
        RenameTarget::Macro(target) => rename_macro(db, file_id, &config, target, new_name),
        RenameTarget::Hdl(ResolvedRenameTarget { selected_def, .. }) => {
            rename_definition(db, &sema, file_id, &config, &selected_def, new_name, None)
        }
    }
}

pub(crate) fn rename_expansion_info(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
) -> RenameResult<RecursiveRenameInfo> {
    let sema = Semantics::new(db);
    let resolved = match resolve_rename_target(&sema, position)? {
        RenameTarget::Macro(_) => {
            // Recursive rename follows same-name port connections; macros have
            // no such semantics.
            return Ok(RecursiveRenameInfo { additional_symbols: 0 });
        }
        RenameTarget::Hdl(target) => target,
    };
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
    match resolve_rename_target(&sema, position)? {
        // Macros have no recursive semantics; the expanded rename is the
        // plain rename.
        RenameTarget::Macro(target) => {
            rename_macro(db, position.file_id, &config, target, new_name)
        }
        RenameTarget::Hdl(resolved) => {
            let targets =
                recursive_rename_targets(db, &sema, position.file_id, &config, resolved.targets)?;
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
    }
}

pub(crate) fn rename_conflict_info(
    db: &RootDb,
    position: FilePosition,
    config: RenameConfig,
    new_name: &str,
    recursive: bool,
) -> RenameResult<RenameCollisionInfo> {
    let sema = Semantics::new(db);
    let resolved = match resolve_rename_target(&sema, position)? {
        // The preproc model has no name-scope query for macros yet; report no
        // collisions for macro renames.
        RenameTarget::Macro(_) => return Ok(RenameCollisionInfo { conflicts: 0 }),
        RenameTarget::Hdl(target) => target,
    };
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

enum RenameTarget {
    Hdl(ResolvedRenameTarget),
    Macro(ResolvedMacroTarget),
}

impl RenameTarget {
    fn range(&self) -> TextRange {
        match self {
            RenameTarget::Hdl(target) => target.range,
            RenameTarget::Macro(target) => target.range,
        }
    }
}

struct ResolvedRenameTarget {
    range: TextRange,
    selected_def: DefId,
    targets: Vec<DefId>,
}

struct ResolvedMacroTarget {
    range: TextRange,
    kind: MacroRenameKind,
}

enum MacroRenameKind {
    /// ``define NAME(...)``: rename the definition name and every call site.
    Definition(MacroDefinition),
    /// A macro parameter: rename the header parameter and every body usage.
    Param(MacroParamDefinition),
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
) -> RenameResult<RenameTarget> {
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let target = resolve_semantic_target(
        sema.db,
        file_id,
        offset,
        parsed_file.root(),
        rename_token_precedence,
    );
    match target.unique_for_intent(TargetIntent::Rename).ok_or(RenameError::NoRefFound)? {
        SemanticTarget::PreprocMacro(target) => resolve_macro_rename_target(target),
        SemanticTarget::Include(_) => Err(RenameError::NoRefFound),
        SemanticTarget::Source(target) => {
            resolve_hdl_rename_target(sema, hir_file_id, target).map(RenameTarget::Hdl)
        }
    }
}

/// The macro target behind a rename position: the definition or parameter
/// the caret names, deduplicated across the workspace models.
fn resolve_macro_rename_target(target: PreprocMacroTarget) -> RenameResult<RenameTarget> {
    let target = match target {
        PreprocMacroTarget::ParamDefinition(definition) => ResolvedMacroTarget {
            range: definition.range,
            kind: MacroRenameKind::Param(definition),
        },
        PreprocMacroTarget::ParamReference(resolution) => {
            let Some(definition) = unique_macro_param_definition(&resolution.definitions) else {
                return Err(RenameError::NoDefFound);
            };
            ResolvedMacroTarget {
                range: resolution.range,
                kind: MacroRenameKind::Param(definition),
            }
        }
        PreprocMacroTarget::Definition(definition) => ResolvedMacroTarget {
            range: definition.name_range,
            kind: MacroRenameKind::Definition(definition),
        },
        PreprocMacroTarget::Reference(resolution) => {
            let Some(definition) = unique_macro_definition(&resolution.definitions) else {
                return Err(RenameError::NoDefFound);
            };
            ResolvedMacroTarget {
                range: resolution.range,
                kind: MacroRenameKind::Definition(definition),
            }
        }
    };
    Ok(RenameTarget::Macro(target))
}

/// The definition behind a rename position must be unique: several same-name
/// macros (or predefines from multiple manifests) are distinct definitions
/// that must not be renamed together.
fn unique_macro_definition(definitions: &[MacroDefinition]) -> Option<MacroDefinition> {
    (definitions.len() == 1).then(|| definitions[0].clone())
}

fn unique_macro_param_definition(
    definitions: &[MacroParamDefinition],
) -> Option<MacroParamDefinition> {
    (definitions.len() == 1).then(|| definitions[0].clone())
}

fn resolve_hdl_rename_target(
    sema: &Semantics<'_, RootDb>,
    hir_file_id: HirFileId,
    target: SourceTarget<'_>,
) -> RenameResult<ResolvedRenameTarget> {
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

/// Renames a macro definition or parameter: the defining token plus every
/// reference from the preproc reference index.
fn rename_macro(
    db: &RootDb,
    request_file_id: FileId,
    config: &RenameConfig,
    target: ResolvedMacroTarget,
    new_name: &str,
) -> RenameResult<SourceChange> {
    let mut source_changes = SourceChange::default();
    let mut insert = |file_id, range| {
        source_changes
            .insert_text_edit(file_id, TextEdit::replace(range, new_name.to_owned()))
            .map_err(|_| RenameError::OverlappingEdits)
    };

    match target.kind {
        MacroRenameKind::Definition(definition) => {
            if config.edit_scope == RenameEditScope::SingleFile
                && (definition.file_id != request_file_id)
            {
                return Err(RenameError::ProjectScopeRequired);
            }
            let refs = macro_references(db, request_file_id, &definition)
                .map_err(RenameError::MacroRenameFailed)?;
            insert(definition.file_id, definition.name_range)?;
            for reference in refs.references {
                if config.edit_scope == RenameEditScope::SingleFile
                    && reference.file_id != request_file_id
                {
                    return Err(RenameError::ProjectScopeRequired);
                }
                let range = macro_reference_name_range(db, &reference);
                insert(reference.file_id, range)?;
            }
        }
        MacroRenameKind::Param(definition) => {
            if config.edit_scope == RenameEditScope::SingleFile
                && (definition.macro_definition.file_id != request_file_id)
            {
                return Err(RenameError::ProjectScopeRequired);
            }
            let refs = macro_param_references(db, request_file_id, &definition)
                .map_err(RenameError::MacroRenameFailed)?;
            insert(definition.macro_definition.file_id, definition.range)?;
            for reference in refs.references {
                if config.edit_scope == RenameEditScope::SingleFile
                    && reference.file_id != request_file_id
                {
                    return Err(RenameError::ProjectScopeRequired);
                }
                insert(reference.file_id, reference.range)?;
            }
        }
    }

    Ok(source_changes)
}

/// The call-site reference range can include the directive backtick
/// (`` `NAME ``); the rename only replaces the name token.
fn macro_reference_name_range(db: &RootDb, reference: &MacroReference) -> TextRange {
    let mut range = reference.range;
    if range.start() < range.end() {
        let text = db.file_text(reference.file_id);
        let start = usize::from(range.start());
        if text.as_bytes().get(start) == Some(&b'`') {
            range =
                TextRange::new(range.start() + utils::text_edit::TextSize::from(1), range.end());
        }
    }
    range
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

/// The connected component of same-name port connections around `def` under
/// `visibility`, optionally restricted to one file, in discovery order. A
/// salsa query so the recursive rename info, conflict and edit commands share
/// one computation across requests.
pub(crate) fn recursive_rename_closure_impl(
    db: &dyn WorkspaceSymbolIndexDb,
    def: DefId,
    visibility: ScopeVisibility,
    single_file: Option<FileId>,
) -> Vec<DefId> {
    let config = ReferencesConfig::new(visibility, single_file.map(SearchScope::single_file));
    let mut targets = UniqVec::<DefId, DefOrigin>::default();
    targets.push(def.origins(db), def);
    let mut idx = 0;
    while idx < targets.len() {
        let current = targets.get(idx).clone();
        idx += 1;
        let scope = SearchScope::new(db, &current, config.clone());
        let refs = search_references(db, &current, scope);
        // Same-name connections connect their paired definition: follow them
        // to close the recursive rename set.
        for toks in refs.values() {
            for token_ref in toks {
                if let Some(paired) = token_ref.context().paired() {
                    targets.push(paired.origins(db), paired.clone());
                }
            }
        }
    }
    targets.into_vec()
}

fn recursive_rename_targets(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: FileId,
    config: &RenameConfig,
    initial_targets: Vec<DefId>,
) -> RenameResult<Vec<RecursiveRenameTarget>> {
    let single_file = match config.edit_scope {
        RenameEditScope::Workspace => None,
        RenameEditScope::SingleFile => Some(file_id),
    };
    let mut targets = UniqVec::<DefId, DefOrigin>::default();
    for target in initial_targets {
        let closure = db.recursive_rename_closure(target, config.scope_visibility, single_file);
        for def in closure.iter() {
            targets.push(def.origins(db), def.clone());
        }
    }
    let mut resolved_targets = Vec::new();
    for def in targets.into_vec() {
        let refs = references_for_definition(db, sema, file_id, config, &def)?;
        resolved_targets.push(RecursiveRenameTarget { def, refs });
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

    #[test]
    fn macro_definition_rename_updates_definition_and_call_sites() {
        check_rename(
            "`define /*caret*/FOO(x) x\nmodule top;\n  logic a;\n  assign y = `FOO(a);\nendmodule\n",
            "BAR",
            "`define BAR(x) x\nmodule top;\n  logic a;\n  assign y = `BAR(a);\nendmodule\n",
        );
    }

    #[test]
    fn macro_definition_rename_from_call_site() {
        check_rename(
            "`define FOO(x) x\nmodule top;\n  logic a;\n  assign y = `/*caret*/FOO(a);\nendmodule\n",
            "BAR",
            "`define BAR(x) x\nmodule top;\n  logic a;\n  assign y = `BAR(a);\nendmodule\n",
        );
    }

    #[test]
    fn macro_param_rename_updates_header_and_body_usages() {
        check_rename(
            "`define FOO(/*caret*/x) x + x\nmodule top;\n  assign y = `FOO(1);\nendmodule\n",
            "y",
            "`define FOO(y) y + y\nmodule top;\n  assign y = `FOO(1);\nendmodule\n",
        );
    }

    #[test]
    fn macro_param_rename_from_body_usage() {
        check_rename(
            "`define FOO(x) /*caret*/x + x\nmodule top;\n  assign y = `FOO(1);\nendmodule\n",
            "y",
            "`define FOO(y) y + y\nmodule top;\n  assign y = `FOO(1);\nendmodule\n",
        );
    }

    #[test]
    fn macro_rename_has_no_recursive_expansion_or_conflicts() {
        let (db, file_id, offset) = db_with_caret(
            "`define /*caret*/FOO(x) x\nmodule top;\n  assign y = `FOO(1);\nendmodule\n",
        );
        let config = RenameConfig::workspace(ScopeVisibility::Public);
        let position = FilePosition { file_id, offset };
        let info = rename_expansion_info(&db, position, config.clone()).unwrap();
        assert_eq!(info.additional_symbols, 0);
        let conflicts = rename_conflict_info(&db, position, config, "BAR", false).unwrap();
        assert_eq!(conflicts.conflicts, 0);
    }
}
