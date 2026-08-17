use base_db::source_root::SourceRootId;
use hir_def::{
    container::InFile,
    def_id::DefId,
    has_source::HasSource,
    module::ModuleKind,
    owner::{OwnerId, OwnerKind},
};
use hir_semantics::semantics::SemanticsImpl;
use hir_ty::db::TyDb;
use nohash_hasher::IntMap;
use preproc_expand::{file::HirFileId, macro_file::macro_file_call_site};
use rustc_hash::FxHashMap;
use syntax::{SyntaxTokenWithParent, has_text_range::HasTextRange, ptr::SyntaxTokenPtr};
use utils::line_index::TextRange;
use vfs::FileId;

use super::{ReferenceCategory, ReferencesConfig};
use crate::{
    ScopeVisibility,
    analysis::AnalysisContext,
    db::workspace_symbol_index_db::WorkspaceSymbolIndexDb,
    definitions::DefinitionClass,
    semantic_index::{
        ReferenceContext,
        build::{
            ContainerCache, ScopeChainCache, definition_class_for_token, definition_ranges_for,
            reference_context, token_in_special_context,
        },
    },
    semantic_target::preproc::{EmittedTokenIndex, emit_token_index},
};

/// A search scope is a set of files and ranges within those files that should
/// be searched. None means the whole file.
#[derive(Default, Debug, Clone)]
pub struct SearchScope(FxHashMap<FileId, Option<TextRange>>);

impl SearchScope {
    pub(crate) fn single_file(file_id: FileId) -> Self {
        let res = FxHashMap::from_iter([(file_id, None)]);
        SearchScope(res)
    }

    pub(crate) fn new(
        db: &dyn WorkspaceSymbolIndexDb,
        def: &DefId,
        ReferencesConfig { scope_visibility, search_scope }: ReferencesConfig,
    ) -> Self {
        match scope_visibility {
            ScopeVisibility::Public => search_scope.unwrap_or_else(|| Self::all(db)),
            ScopeVisibility::Private => {
                let container_id = def.container_id(db);
                // Ports and package members are visible beyond their module
                // or package block (the whole compilation unit), so a private
                // scope must not cut their references off at the block.
                let container_id = if container_id.kind(db) == OwnerKind::Module
                    && (def.is_port(db)
                        || container_id.module_kind(db) == Some(ModuleKind::Package))
                {
                    db.owner_table(container_id.file(db)).file_owner().expect("file owner")
                } else {
                    container_id
                };

                let mut scope = Self::from_conts(db, container_id);

                if let Some(search_scope) = search_scope {
                    scope = scope.intersect(search_scope);
                }

                scope
            }
        }
    }

    pub(crate) fn all(db: &dyn WorkspaceSymbolIndexDb) -> Self {
        let res = db.files().iter().map(|&file_id| (file_id, None)).collect();
        SearchScope(res)
    }

    fn single_range(file_id: FileId, range: TextRange) -> Self {
        let res = FxHashMap::from_iter([(file_id, Some(range))]);
        SearchScope(res)
    }

    /// Like [`single_range`](Self::single_range) but resolves a HIR file
    /// location to its user-facing source file and range first. Macro
    /// expansions map to their call site; when a call site cannot be resolved
    /// the search falls back to the whole workspace.
    fn single_source_range(
        db: &dyn WorkspaceSymbolIndexDb,
        file_id: HirFileId,
        range: TextRange,
    ) -> Self {
        match resolve_source_range(db, file_id, range) {
            Some((file_id, range)) => Self::single_range(file_id, range),
            None => Self::all(db),
        }
    }

    fn from_conts(db: &dyn WorkspaceSymbolIndexDb, cont: OwnerId) -> Self {
        if cont.kind(db) == OwnerKind::File {
            return Self::all(db);
        }

        let Some(InFile { file_id, value: source }) = cont.source(db) else {
            return Self::all(db);
        };
        Self::single_source_range(db, file_id, source.full_range())
    }

    fn intersect(mut self, mut other: SearchScope) -> SearchScope {
        if self.0.len() > other.0.len() {
            std::mem::swap(&mut self, &mut other)
        }

        self.0.retain(|file_id, range| {
            let Some(other_range) = other.0.get(file_id) else {
                return false;
            };

            match (&range, &other_range) {
                (Some(r), Some(other)) => *range = r.intersect(*other),
                (None, Some(other)) => *range = Some(*other),
                (Some(_), None) | (None, None) => {}
            };

            true
        });

        self
    }

    pub(crate) fn is_within_file(&self, file_id: FileId) -> bool {
        self.0.keys().all(|candidate| *candidate == file_id)
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = FileId> + '_ {
        self.0.keys().copied()
    }

    /// The single file of the scope, if it covers exactly one file.
    pub(crate) fn single_file_id(&self) -> Option<FileId> {
        let mut keys = self.0.keys();
        let first = keys.next()?;
        keys.next().is_none().then_some(*first)
    }

    pub(crate) fn range_for_file(&self, file_id: FileId) -> Option<Option<TextRange>> {
        self.0.get(&file_id).copied()
    }

    pub(crate) fn contains(&self, file_id: FileId, range: TextRange) -> bool {
        self.range_for_file(file_id).is_some_and(|file_range| {
            file_range.is_none_or(|file_range| file_range.intersect(range).is_some())
        })
    }

    fn source_root_ids(&self, db: &dyn WorkspaceSymbolIndexDb) -> Vec<SourceRootId> {
        let mut root_ids =
            self.0.keys().map(|file_id| db.source_root_id(*file_id)).collect::<Vec<_>>();
        root_ids.sort_unstable();
        root_ids.dedup();
        root_ids
    }
}

pub(crate) struct ReferencesCtx<'a> {
    db: &'a AnalysisContext<'a>,
    def: DefId,
    scope: SearchScope,
}

#[derive(Debug, Clone)]
pub(crate) struct ReferenceToken {
    ptr: SyntaxTokenPtr,
    range: TextRange,
    category: ReferenceCategory,
    context: ReferenceContext,
}

impl ReferenceToken {
    pub fn range(&self) -> TextRange {
        self.range
    }

    pub fn category(&self) -> ReferenceCategory {
        self.category
    }

    pub(crate) fn context(&self) -> &ReferenceContext {
        &self.context
    }

    pub fn to_token<'a>(&self, tree: &'a syntax::SyntaxTree) -> Option<SyntaxTokenWithParent<'a>> {
        self.ptr.to_token(tree)
    }
}

impl<'a> ReferencesCtx<'a> {
    const FILE_REF_CAPACITY: usize = 8;

    pub(crate) fn new(db: &'a AnalysisContext<'a>, def: &DefId, cfg: ReferencesConfig) -> Self {
        let scope = SearchScope::new(db.db, def, cfg);
        Self { db, def: *def, scope }
    }

    pub(crate) fn search(&self) -> IntMap<FileId, Vec<ReferenceToken>> {
        search_references(self.db, &self.def, self.scope.clone())
    }
}

/// Collects the references of `def` inside `scope`.
///
/// Candidate files come from the name occurrence table. Each candidate is
/// resolved on demand; the workspace product never stores a `DefId` map.
pub(crate) fn search_references(
    db: &AnalysisContext<'_>,
    def: &DefId,
    scope: SearchScope,
) -> IntMap<FileId, Vec<ReferenceToken>> {
    let mut res: IntMap<_, Vec<_>> = IntMap::default();
    let Some(name) = def.name(db.db) else {
        return res;
    };

    if let Some(file_id) = scope.single_file_id() {
        db.unwind_if_revision_cancelled();
        collect_file_references(db, file_id, def, &name, &scope, &mut res);
        return res;
    }

    for source_root_id in scope.source_root_ids(db.db) {
        db.unwind_if_revision_cancelled();
        let index = db.name_index(source_root_id);
        for &file_id in index.files_mentioning(&name) {
            if scope.range_for_file(file_id).is_none() {
                continue;
            }
            db.unwind_if_revision_cancelled();
            collect_file_references(db, file_id, def, &name, &scope, &mut res);
        }
    }

    res
}

fn collect_file_references(
    db: &AnalysisContext<'_>,
    file_id: FileId,
    def: &DefId,
    name: &str,
    scope: &SearchScope,
    res: &mut IntMap<FileId, Vec<ReferenceToken>>,
) {
    let file_index = db.file_name_index(file_id);
    let occurrences = file_index.occurrences(name);
    if occurrences.is_empty() {
        return;
    }

    let context = db.semantic_snapshot_inputs();
    let hir_file_id = HirFileId::from(file_id);
    let tree = db.parse_file(file_id);
    let emitted = emit_token_index(tree.root());
    let text = db.file_text(file_id);
    let sema = SemanticsImpl::new_with_context(db.db, context.hir.clone());
    let mut containers = ContainerCache::new();
    let mut chains = ScopeChainCache::new();
    let mut conn_port_by_name = FxHashMap::default();
    let definition_ranges = definition_ranges_for(db.db, *def);

    for occurrence in occurrences {
        if !scope.contains(file_id, occurrence.range) {
            continue;
        }
        if definition_ranges.iter().any(|definition_range| {
            definition_range.file_id == file_id && definition_range.range == occurrence.range
        }) {
            continue;
        }
        let Some(token) = token_for_occurrence(&tree, &emitted, occurrence) else {
            continue;
        };
        let container = containers.container_for(&sema, hir_file_id, token.parent);
        let Some(class) = definition_class_for_token(
            db.db,
            &sema,
            &context,
            hir_file_id,
            token,
            container,
            token_in_special_context(token),
            &mut chains,
        ) else {
            continue;
        };

        let sides = match &class {
            DefinitionClass::Definition(found) if found == def => {
                &[crate::semantic_index::ConnSide::Port][..]
            }
            DefinitionClass::PortConnShorthand { port, local } if port == def || local == def => {
                if port == def {
                    &[crate::semantic_index::ConnSide::Port][..]
                } else {
                    &[crate::semantic_index::ConnSide::Local][..]
                }
            }
            _ => continue,
        };

        for &side in sides {
            let reference_context = reference_context(
                db.db,
                &sema,
                &context,
                hir_file_id,
                token,
                &class,
                container,
                &mut chains,
                &mut conn_port_by_name,
                &text,
                side,
            );
            let tokens = res
                .entry(file_id)
                .or_insert_with(|| Vec::with_capacity(ReferencesCtx::FILE_REF_CAPACITY));
            if tokens.iter().any(|existing| existing.range == occurrence.range) {
                continue;
            }
            tokens.push(ReferenceToken {
                ptr: SyntaxTokenPtr::from_token(token),
                range: occurrence.range,
                category: ReferenceCategory::from_tok(token),
                context: reference_context,
            });
        }
    }
}

pub(crate) fn token_for_occurrence<'tree>(
    tree: &'tree syntax::SyntaxTree,
    emitted: &EmittedTokenIndex<'tree>,
    occurrence: &crate::name_index::NameOccurrence,
) -> Option<SyntaxTokenWithParent<'tree>> {
    if let Some(emitted_id) = occurrence.emitted
        && let Some(token) = emitted.get(&emitted_id).and_then(|tokens| {
            tokens.iter().copied().find(|token| {
                token.kind() == occurrence.kind && token.text_range() == Some(occurrence.range)
            })
        })
    {
        return Some(token);
    }
    // L0 extract and the request parse can disagree on emitted indices when
    // includes expand. Fall back to (kind, range) rather than dropping the hit.
    SyntaxTokenPtr::from_kind_range(occurrence.kind, occurrence.range).to_token(tree)
}

/// Resolves a HIR file location to a user-facing source file and range.
///
/// For real files the location is returned as-is. For macro expansions the
/// location is mapped to the macro invocation site, since the expanded text is
/// not a file the user can open. Returns `None` when a macro expansion's call
/// site cannot be resolved.
pub(crate) fn resolve_source_range(
    db: &dyn TyDb,
    file_id: HirFileId,
    range: TextRange,
) -> Option<(FileId, TextRange)> {
    match file_id {
        HirFileId::File(file_id) => Some((file_id, range)),
        HirFileId::Macro(macro_file) => {
            let call_site = macro_file_call_site(db, macro_file)?;
            Some((call_site.call_file_id, call_site.call_range))
        }
    }
}
