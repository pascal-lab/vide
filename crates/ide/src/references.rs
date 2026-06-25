use hir::{file::HirFileId, semantics::Semantics};
use nohash_hasher::IntMap;
use search::SearchScope;
use syntax::{SyntaxTokenWithParent, has_text_range::HasTextRange, token::pair_token};
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    ScopeVisibility, db::root_db::RootDb, definitions::Definition, navigation_target::NavTarget,
};

pub(crate) mod search;

bitflags::bitflags! {
    #[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Debug)]
    pub struct ReferenceCategory: u8 {
        const WRITE = 1 << 0;
        const READ = 1 << 1;
    }
}

impl ReferenceCategory {
    pub fn from_tok(SyntaxTokenWithParent { .. }: SyntaxTokenWithParent) -> ReferenceCategory {
        // TODO:
        ReferenceCategory::empty()
    }
}

#[derive(Debug, Clone)]
pub struct ReferencesConfig {
    pub scope_visibility: ScopeVisibility,
    pub search_scope: Option<SearchScope>,
}

impl ReferencesConfig {
    pub fn new(scope_visibility: ScopeVisibility, search_scope: Option<SearchScope>) -> Self {
        Self { scope_visibility, search_scope }
    }

    pub(crate) fn search_scope(&self, db: &RootDb, def: &Definition) -> SearchScope {
        SearchScope::new(db, def, self.clone())
    }
}

#[derive(Debug, Clone)]
pub struct References {
    pub def: Option<Vec<NavTarget>>,
    pub refs: IntMap<FileId, Vec<(TextRange, ReferenceCategory)>>,
    pub status: ReferencesStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferencesStatus {
    Complete,
    Partial { reason: ReferencesPartialReason, issue_count: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferencesPartialReason {
    PreprocMacroIndex,
}

impl ReferencesStatus {
    pub fn is_partial(self) -> bool {
        matches!(self, ReferencesStatus::Partial { .. })
    }

    pub fn issue_count(self) -> usize {
        match self {
            ReferencesStatus::Complete => 0,
            ReferencesStatus::Partial { issue_count, .. } => issue_count,
        }
    }
}

pub(crate) fn handle_ctrl_flow_kw(
    _sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { .. }: SyntaxTokenWithParent,
) -> Option<Vec<References>> {
    let kind = tp.kind();

    let mut refs = vec![];
    let mut add_ref = |tok: SyntaxTokenWithParent| {
        if let Some(range) = tok.text_range() {
            refs.push((range, ReferenceCategory::empty()));
        }
    };

    match kind {
        _ if let Some(pair) = pair_token(tp) => {
            let pair = pair.either(|tok| tok, |tok| tok);
            add_ref(tp);
            add_ref(pair);
        }
        _ => return None,
    }

    Some(vec![References {
        def: None,
        refs: IntMap::from_iter([(file_id.file_id(), refs)]),
        status: ReferencesStatus::Complete,
    }])
}
