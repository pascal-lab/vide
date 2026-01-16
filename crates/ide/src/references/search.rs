use std::cell::LazyCell;

use base_db::{intern::Lookup, salsa::Database, source_db::SourceDb};
use hir::{
    container::{ContainerId, InFile},
    semantics::Semantics,
    source_map::IsSrc,
};
use ide_db::root_db::RootDb;
use itertools::Itertools;
use memchr::memmem::Finder;
use nohash_hasher::IntMap;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, ast::AstNode, has_text_range::HasTextRange,
    token::TokenKindExt,
};
use triomphe::Arc;
use utils::{
    get::Get,
    line_index::{TextRange, TextSize},
};
use vfs::FileId;

use super::{ReferenceCategory, ReferencesConfig};
use crate::{
    ScopeVisibility,
    definitions::{Definition, DefinitionClass},
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

    fn new(
        db: &RootDb,
        def: &Definition,
        ReferencesConfig { scope_visibility, search_scope }: ReferencesConfig,
    ) -> Self {
        match scope_visibility {
            ScopeVisibility::Public => search_scope.unwrap_or_else(|| Self::all(db)),
            ScopeVisibility::Private => {
                let container_id = match def.container_id(db) {
                    ContainerId::ModuleId(InFile { file_id, .. }) if def.is_port() => {
                        file_id.into()
                    }
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

    fn from_conts(db: &RootDb, cont: ContainerId) -> Self {
        match cont {
            ContainerId::HirFileId(_) => Self::all(db),
            ContainerId::ModuleId(InFile { value: local_module_id, file_id }) => {
                let range = file_id.to_container_src_map(db).get(local_module_id).range();
                Self::single_range(file_id.file_id(), range)
            }
            ContainerId::BlockId(block_id) => {
                let range = block_id.lookup(db).src.value.range();
                Self::single_range(block_id.file_id(db), range)
            }
            ContainerId::SubroutineId(_) | ContainerId::FileSubroutineId(_) => {
                // TODO: implement search scope for these container types
                Self::all(db)
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
}

pub(crate) struct ReferencesCtx<'a, 'b> {
    sema: &'a Semantics<'a, RootDb>,
    def: &'b Definition,
    scope: SearchScope,
}

#[derive(Debug, Clone)]
pub(crate) struct ReferenceToken<'a> {
    pub token: SyntaxTokenWithParent<'a>,
}

impl ReferenceToken<'_> {
    pub fn range(&self) -> TextRange {
        self.token.text_range().unwrap()
    }

    pub fn category(&self) -> ReferenceCategory {
        ReferenceCategory::from_tok(self.token)
    }
}

impl<'a, 'b> ReferencesCtx<'a, 'b> {
    const FILE_REF_CAPACITY: usize = 8;

    pub(crate) fn new(
        sema: &'a Semantics<'a, RootDb>,
        def: &'b Definition,
        cfg: ReferencesConfig,
    ) -> Self {
        let scope = SearchScope::new(sema.db, def, cfg);
        Self { sema, def, scope }
    }

    pub(crate) fn search(&self) -> IntMap<FileId, Vec<ReferenceToken<'a>>> {
        let sema = self.sema;
        let db = sema.db;
        let mut res: IntMap<_, Vec<_>> = IntMap::default();

        let name = self.def.origins().into_iter().map(|def| def.name(db)).next().unwrap();
        debug_assert! {{
            let names = self.def.origins().into_iter().map(|def| def.name(sema.db)).collect_vec();
            !names.is_empty() && names.iter().all(|namei| namei == &name)
        }};

        let def_ranges: SmallVec<[_; 6]> =
            self.def.origins().into_iter().map(|def| def.name_range(db)).collect();

        let finder = &Finder::new(&name);
        for (text, file_id, range) in self.scope_files() {
            self.sema.db.unwind_if_cancelled();

            let root = LazyCell::new(|| sema.parse(file_id).syntax());
            Self::match_text(&text, finder, range)
                .filter_map(|offset| Self::filter_token(*root, file_id, &def_ranges, offset))
                .filter(|tp| self.classify_and_filter(sema, tp))
                .for_each(|token| {
                    res.entry(file_id)
                        .or_insert_with(|| Vec::with_capacity(Self::FILE_REF_CAPACITY))
                        .push(ReferenceToken { token })
                });
        }

        res
    }

    fn scope_files(&self) -> impl Iterator<Item = (Arc<str>, FileId, TextRange)> + '_ {
        self.scope.0.iter().map(|(file_id, range)| {
            let text = self.sema.db.file_text(*file_id);
            let range = range.unwrap_or_else(|| TextRange::up_to(TextSize::of(&*text)));
            (text, *file_id, range)
        })
    }

    fn match_text<'c>(
        text: &'c str,
        finder: &'c Finder,
        search_range: TextRange,
    ) -> impl Iterator<Item = TextSize> + 'c {
        finder.find_iter(text.as_bytes()).filter_map(move |idx| {
            let offset = TextSize::from(idx as u32);
            if !search_range.contains_inclusive(offset) {
                return None;
            }

            // If this is not a word boundary, that means this is only part of an ident.
            if text[..idx].chars().next_back().is_some_and(|ch| ch.is_alphabetic() || ch == '_')
                || text[idx + finder.needle().len()..]
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
            {
                return None;
            }

            Some(offset)
        })
    }

    fn filter_token(
        node: SyntaxNode<'a>,
        file_id: FileId,
        names: &[InFile<TextRange>],
        offset: TextSize,
    ) -> Option<SyntaxTokenWithParent<'a>> {
        let tok = node.token_at_offset(offset).find(|tok| tok.kind().name_like())?;

        // filter out definitions
        if names.iter().any(|InFile { value: range, file_id: name_file_id }| {
            &tok.text_range().unwrap() == range && *name_file_id == file_id.into()
        }) {
            None
        } else {
            Some(tok)
        }
    }

    fn classify_and_filter(
        &self,
        sema: &'a Semantics<'a, RootDb>,
        tp: &SyntaxTokenWithParent<'a>,
    ) -> bool {
        let Some(def) = DefinitionClass::resolve(sema, *tp) else {
            return false;
        };

        match def {
            DefinitionClass::Definition(def) => def == *self.def,
            DefinitionClass::PortConnShorthand { local, port } => {
                local == *self.def || port == *self.def
            }
        }
    }
}
