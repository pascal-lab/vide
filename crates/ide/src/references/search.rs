use hir::{
    base_db::{
        intern::Lookup,
        salsa::Database,
        source_db::{SourceDb, SourceRootDb},
        source_root::SourceRootId,
    },
    container::{InFile, ScopeId},
    def_id::DefId,
    semantics::Semantics,
    source_map::IsSrc,
    symbol::DefOrigin,
};
use nohash_hasher::IntMap;
use rustc_hash::FxHashMap;
use syntax::{SyntaxTokenWithParent, ptr::SyntaxTokenPtr};
use utils::{get::Get, line_index::TextRange};
use vfs::FileId;

use super::{ReferenceCategory, ReferencesConfig};
use crate::{
    ScopeVisibility,
    db::{root_db::RootDb, workspace_symbol_index_db::source_root_semantic_index_for_root},
    semantic_index::SemanticReference,
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
        db: &RootDb,
        def: &DefId,
        ReferencesConfig { scope_visibility, search_scope }: ReferencesConfig,
    ) -> Self {
        match scope_visibility {
            ScopeVisibility::Public => search_scope.unwrap_or_else(|| Self::all(db)),
            ScopeVisibility::Private => {
                let container_id = def.container_id(db);
                let container_id = match container_id {
                    ScopeId::Module(InFile { file_id, .. }) if def.is_port(db) => file_id.into(),
                    cont => cont,
                };

                let mut scope = Self::from_conts(db, container_id);

                if let Some(search_scope) = search_scope {
                    scope = scope.intersect(search_scope);
                }

                scope
            }
        }
    }

    pub(crate) fn all(db: &RootDb) -> Self {
        let res = db.files().iter().map(|&file_id| (file_id, None)).collect();
        SearchScope(res)
    }

    fn single_range(file_id: FileId, range: TextRange) -> Self {
        let res = FxHashMap::from_iter([(file_id, Some(range))]);
        SearchScope(res)
    }

    fn from_conts(db: &RootDb, cont: ScopeId) -> Self {
        match cont {
            ScopeId::File(_) => Self::all(db),
            ScopeId::Module(InFile { value: local_module_id, file_id }) => {
                if let Some(range) =
                    file_id.to_container_src_map(db).get(local_module_id).map(|src| src.range())
                {
                    Self::single_range(file_id.file_id(), range)
                } else {
                    Self::all(db)
                }
            }
            ScopeId::Block(block_id) => {
                let range = block_id.lookup(db).src.value.range();
                Self::single_range(block_id.file_id(db), range)
            }
            ScopeId::GenerateBlock(generate_block_id) => {
                let src = generate_block_id.lookup(db).src;
                Self::single_range(src.file_id.file_id(), src.value.range())
            }
            ScopeId::Subroutine(subroutine_id) => {
                let def_id = DefOrigin::new(db, subroutine_id.as_in_container());
                if let Some(InFile { file_id, value: range }) = def_id.range(db) {
                    Self::single_range(file_id.file_id(), range)
                } else {
                    Self::all(db)
                }
            }
            ScopeId::ClockingBlock(clocking_block_id) => {
                let def_id = DefOrigin::new(db, clocking_block_id);
                if let Some(InFile { file_id, value: range }) = def_id.range(db) {
                    Self::single_range(file_id.file_id(), range)
                } else {
                    Self::all(db)
                }
            }
            ScopeId::Checker(checker_id) => {
                let def_id = DefOrigin::new(db, checker_id.as_in_container());
                if let Some(InFile { file_id, value: range }) = def_id.range(db) {
                    Self::single_range(file_id.file_id(), range)
                } else {
                    Self::all(db)
                }
            }
            ScopeId::Covergroup(covergroup_id) => {
                let def_id = DefOrigin::new(db, covergroup_id.as_in_container());
                if let Some(InFile { file_id, value: range }) = def_id.range(db) {
                    Self::single_range(file_id.file_id(), range)
                } else {
                    Self::all(db)
                }
            }
        }
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

    pub(crate) fn range_for_file(&self, file_id: FileId) -> Option<Option<TextRange>> {
        self.0.get(&file_id).copied()
    }

    pub(crate) fn contains(&self, file_id: FileId, range: TextRange) -> bool {
        self.range_for_file(file_id).is_some_and(|file_range| {
            file_range.is_none_or(|file_range| file_range.intersect(range).is_some())
        })
    }

    fn source_root_ids(&self, db: &RootDb) -> Vec<SourceRootId> {
        let mut root_ids =
            self.0.keys().map(|file_id| db.source_root_id(*file_id)).collect::<Vec<_>>();
        root_ids.sort_unstable();
        root_ids.dedup();
        root_ids
    }
}

pub(crate) struct ReferencesCtx<'a, 'b> {
    sema: &'a Semantics<'a, RootDb>,
    def: &'b DefId,
    scope: SearchScope,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReferenceToken {
    ptr: SyntaxTokenPtr,
    range: TextRange,
    category: ReferenceCategory,
}

impl ReferenceToken {
    pub(crate) fn from_semantic_reference(reference: &SemanticReference) -> Self {
        Self { ptr: reference.ptr, range: reference.range, category: reference.category }
    }

    pub fn range(&self) -> TextRange {
        self.range
    }

    pub fn category(&self) -> ReferenceCategory {
        self.category
    }

    pub fn to_token<'a>(self, tree: &'a syntax::SyntaxTree) -> Option<SyntaxTokenWithParent<'a>> {
        self.ptr.to_token(tree)
    }
}

impl<'a, 'b> ReferencesCtx<'a, 'b> {
    const FILE_REF_CAPACITY: usize = 8;

    pub(crate) fn new(
        sema: &'a Semantics<'a, RootDb>,
        def: &'b DefId,
        cfg: ReferencesConfig,
    ) -> Self {
        let scope = SearchScope::new(sema.db, def, cfg);
        Self { sema, def, scope }
    }

    pub(crate) fn search(&self) -> IntMap<FileId, Vec<ReferenceToken>> {
        let db = self.sema.db;
        let mut res: IntMap<_, Vec<_>> = IntMap::default();

        for source_root_id in self.scope.source_root_ids(db) {
            self.sema.db.unwind_if_cancelled();
            let index = source_root_semantic_index_for_root(db, source_root_id);
            let Some(group) = index.references_for_definition(*self.def) else {
                continue;
            };

            for reference in group.references.iter() {
                if !self.scope.contains(reference.file_id, reference.range) {
                    continue;
                }
                res.entry(reference.file_id)
                    .or_insert_with(|| Vec::with_capacity(Self::FILE_REF_CAPACITY))
                    .push(ReferenceToken::from_semantic_reference(reference));
            }
        }

        res
    }
}
