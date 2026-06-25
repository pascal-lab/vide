use hir::{file::HirFileId, semantics::Semantics};
use itertools::Itertools;
use nohash_hasher::IntMap;
use search::{ReferencesCtx, SearchScope};
use syntax::{SyntaxTokenWithParent, has_text_range::HasTextRange, token::pair_token};
use utils::line_index::TextRange;
use vfs::FileId;

use self::preproc::render_preproc_references_target;
use crate::{
    FilePosition, ScopeVisibility,
    db::root_db::RootDb,
    definitions::{Definition, DefinitionClass},
    facts::{
        SemanticFacts, TargetQuery,
        target::{SemanticTarget, TargetIntent, TargetResolution},
    },
    navigation_target::{NavTarget, ToNav},
    source_targets::SourceTarget,
};

mod preproc;
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

pub(crate) fn references(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let target = SemanticFacts::new(db).target_at(TargetQuery {
        file_id,
        offset,
        intent: TargetIntent::FindReferences,
        root: parsed_file.root(),
    });
    render_references_target(db, file_id, &sema, target, config)
}

fn render_references_target(
    db: &RootDb,
    file_id: FileId,
    sema: &Semantics<RootDb>,
    target: TargetResolution<'_>,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    match target.unique_for_intent(TargetIntent::FindReferences)? {
        SemanticTarget::PreprocMacro(target) => {
            render_preproc_references_target(db, file_id, target, &config)
        }
        SemanticTarget::Include(_) => None,
        SemanticTarget::Source(target) => {
            render_source_references_target(sema, file_id, target, config)
        }
    }
}

fn render_source_references_target(
    sema: &Semantics<RootDb>,
    file_id: FileId,
    target: SourceTarget<'_>,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    let hir_file_id = file_id.into();
    let tokens = target.into_tokens();
    let references = tokens
        .into_iter()
        .filter_map(|token| references_for_token(sema, hir_file_id, token, config.clone()))
        .flatten()
        .collect_vec();
    (!references.is_empty()).then_some(references)
}

fn references_for_token(
    sema: &Semantics<RootDb>,
    hir_file_id: HirFileId,
    token: SyntaxTokenWithParent,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    handle_ctrl_flow_kw(sema, hir_file_id, token).or_else(|| {
        let def = match DefinitionClass::resolve(sema, hir_file_id, token)? {
            DefinitionClass::Definition(def) => def,
            DefinitionClass::PortConnShorthand { local, .. } => local,
            DefinitionClass::Ambiguous(_) => return None,
        };
        Some(vec![search_refs(sema, def, config)])
    })
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

fn search_refs<'a>(
    sema: &'a Semantics<'a, RootDb>,
    def: Definition,
    config: ReferencesConfig,
) -> References {
    let refs = ReferencesCtx::new(sema, &def, config)
        .search()
        .into_iter()
        .map(|(file_id, tokens)| {
            let res = tokens.into_iter().map(|token| (token.range(), token.category())).collect();
            (file_id, res)
        })
        .collect();
    let def = def.origins().into_iter().filter_map(|def| def.to_nav(sema.db)).collect_vec().into();
    References { def, refs, status: ReferencesStatus::Complete }
}
