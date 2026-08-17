use base_db::source_db::SourceDb;
use hir_def::{
    decl_shard::{Decl, DeclRole},
    def_id::DefId,
};
use hir_semantics::semantics::Semantics;
use itertools::Itertools;
use nohash_hasher::IntMap;
use preproc_expand::file::HirFileId;
use search::{ReferencesCtx, SearchScope};
use syntax::{SyntaxTokenWithParent, TokenKind, has_text_range::HasTextRange};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use self::preproc::render_preproc_references_target;
use crate::{
    FilePosition, ScopeVisibility,
    analysis::AnalysisContext,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    navigation_target::{NavTarget, ToNav},
    semantic_target::{
        SemanticTarget, SourceTarget, TargetIntent, TargetResolution, resolve_semantic_target,
    },
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

    pub(crate) fn search_scope(&self, db: &RootDb, def: &DefId) -> SearchScope {
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
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    if let Some(refs) = design_unit_references_from_shard(db, file_id, offset, &config) {
        return Some(refs);
    }
    let sema = db.semantics();
    let parsed_file = sema.parse_file(file_id);
    let target =
        resolve_semantic_target(db.db, file_id, offset, parsed_file.root(), token_precedence);
    render_references_target(db, file_id, &sema, target, config)
}

/// Cursor is on a compilation-unit design-unit name. An instantiation of
/// that name is a reference iff this declaration is a `unit_index`
/// candidate for the name.
fn design_unit_references_from_shard(
    db: &AnalysisContext<'_>,
    file_id: FileId,
    offset: TextSize,
    config: &ReferencesConfig,
) -> Option<Vec<References>> {
    let decl = db.file_decl_shard(file_id).design_unit_at(offset)?.clone();
    if !decl.role.is_instantiable_module()
        && !matches!(decl.role, DeclRole::Checker | DeclRole::Covergroup)
    {
        return None;
    }
    let name_range = decl.name_range?;
    let def = vec![NavTarget {
        file_id,
        full_range: name_range,
        focus_range: Some(name_range),
        name: Some(decl.name.clone()),
        kind: design_unit_def_kind(decl.role),
        container_name: None,
        description: None,
    }];
    if !db.unit_index().declares_instantiable(file_id, &decl.name, decl.role, decl.ordinal) {
        return Some(vec![References {
            def: Some(def),
            refs: IntMap::default(),
            status: ReferencesStatus::Complete,
        }]);
    }
    let mut refs = IntMap::default();
    for mention_file in design_unit_instantiation_files(db, config) {
        collect_design_unit_mentions(db, mention_file, &decl, file_id, name_range, &mut refs);
    }
    Some(vec![References { def: Some(def), refs, status: ReferencesStatus::Complete }])
}

fn design_unit_def_kind(role: DeclRole) -> Option<crate::DefKind> {
    match role {
        DeclRole::Module => Some(crate::DefKind::Module),
        DeclRole::Interface => Some(crate::DefKind::Interface),
        DeclRole::Package => Some(crate::DefKind::Package),
        DeclRole::Program => Some(crate::DefKind::Program),
        DeclRole::Checker => Some(crate::DefKind::Checker),
        DeclRole::Covergroup => Some(crate::DefKind::Covergroup),
        _ => None,
    }
}

fn design_unit_instantiation_files(
    db: &AnalysisContext<'_>,
    config: &ReferencesConfig,
) -> Vec<FileId> {
    if let Some(scope) = &config.search_scope {
        return scope.files().collect();
    }
    db.files()
        .iter()
        .copied()
        .filter(|&file| db.file_kind(file).is_semantic_compilation_unit())
        .collect()
}

fn collect_design_unit_mentions(
    db: &AnalysisContext<'_>,
    mention_file: FileId,
    decl: &Decl,
    def_file: FileId,
    name_range: TextRange,
    refs: &mut IntMap<FileId, Vec<(TextRange, ReferenceCategory)>>,
) {
    for instantiation in db.file_decl_shard(mention_file).instantiations.iter() {
        if instantiation.name != decl.name
            || !instantiation_matches_decl(instantiation.role, decl.role)
        {
            continue;
        }
        if mention_file == def_file && instantiation.range == name_range {
            continue;
        }
        refs.entry(mention_file)
            .or_default()
            .push((instantiation.range, ReferenceCategory::empty()));
    }
}

fn instantiation_matches_decl(instantiation: DeclRole, decl: DeclRole) -> bool {
    match decl {
        DeclRole::Module | DeclRole::Interface | DeclRole::Program | DeclRole::Covergroup => {
            instantiation == DeclRole::Module
        }
        DeclRole::Checker => instantiation == DeclRole::Checker,
        _ => false,
    }
}

fn render_references_target(
    db: &AnalysisContext<'_>,
    file_id: FileId,
    sema: &Semantics<RootDb>,
    target: TargetResolution<'_>,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    match target.unique_for_intent(TargetIntent::FindReferences)? {
        SemanticTarget::PreprocMacro(target) => {
            render_preproc_references_target(db.db, file_id, target, &config)
        }
        SemanticTarget::Include(_) => None,
        SemanticTarget::Manifest(target) => crate::manifest::references_target(db, target, config),
        SemanticTarget::Source(target) => {
            render_source_references_target(db, sema, file_id, target, config)
        }
    }
}

fn render_source_references_target(
    db: &AnalysisContext<'_>,
    sema: &Semantics<RootDb>,
    file_id: FileId,
    target: SourceTarget<'_>,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    let hir_file_id = file_id.into();
    let tokens = target.into_tokens();
    let references = tokens
        .into_iter()
        .filter_map(|token| references_for_token(db, sema, hir_file_id, token, config.clone()))
        .flatten()
        .collect_vec();
    (!references.is_empty()).then_some(references)
}

fn references_for_token(
    db: &AnalysisContext<'_>,
    sema: &Semantics<RootDb>,
    hir_file_id: HirFileId,
    token: SyntaxTokenWithParent,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    handle_ctrl_flow_kw(sema, hir_file_id, token).or_else(|| {
        let def = match DefinitionClass::resolve(db, hir_file_id, token).unique()? {
            DefinitionClass::Definition(def) => def,
            DefinitionClass::PortConnShorthand { local, .. } => local,
        };
        Some(vec![search_refs(db, def, config)])
    })
}

pub(crate) fn handle_ctrl_flow_kw(
    _sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { .. }: SyntaxTokenWithParent,
) -> Option<Vec<References>> {
    let (beg, end) = crate::token::ctrl_flow_pair(tp)?;

    let mut refs = vec![];
    let mut add_ref = |tok: SyntaxTokenWithParent| {
        if let Some(range) = tok.text_range() {
            refs.push((range, ReferenceCategory::empty()));
        }
    };
    add_ref(beg);
    add_ref(end);

    Some(vec![References {
        def: None,
        refs: IntMap::from_iter([(file_id.expect_file(), refs)]),
        status: ReferencesStatus::Complete,
    }])
}

fn search_refs(db: &AnalysisContext<'_>, def: DefId, config: ReferencesConfig) -> References {
    let refs = ReferencesCtx::new(db, &def, config)
        .search()
        .into_iter()
        .map(|(file_id, tokens)| {
            let res = tokens.into_iter().map(|token| (token.range(), token.category())).collect();
            (file_id, res)
        })
        .collect();
    let def = def.origins(db.db).iter().filter_map(|def| def.to_nav(db.db)).collect_vec().into();
    References { def, refs, status: ReferencesStatus::Complete }
}

fn token_precedence(kind: TokenKind) -> usize {
    crate::token::navigation_precedence(kind)
}
