use hir::{
    base_db::source_db::{SourceDb, SourcePreprocContextStatus, SourceRootDb},
    file::HirFileId,
    semantics::Semantics,
    source_resolver::{PositionResolver, source_graph_model_file_ids_for_file},
};
use itertools::Itertools;
use nohash_hasher::IntMap;
use search::{ReferencesCtx, SearchScope};
use source_model::{
    EntityId, FilePosition as SourceFilePosition, FileRange, ResolvedSourceTarget, SourceEntity,
    SourceGraph, SourcePurpose, SourceRangeResult, SourceTarget as GraphSourceTarget,
    SourceTargetResolution as GraphSourceTargetResolution,
};
use syntax::{
    SyntaxNode, SyntaxTokenWithParent, TokenKind,
    has_text_range::HasTextRange,
    token::{TokenKindExt, pair_token},
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    FilePosition, ScopeVisibility,
    db::root_db::RootDb,
    definitions::{Definition, DefinitionClass},
    navigation_target::{NavTarget, ToNav},
    syntax_targets::{SyntaxTarget, generated_syntax_target_at_offset, syntax_target_at_offset},
};

pub(crate) mod search;

enum ReferencesTarget<'tree> {
    Preproc(PreprocReferencesTarget),
    Source(SyntaxTarget<'tree>),
}

#[derive(Clone, Copy)]
struct GraphEntityTarget {
    model_file_id: FileId,
    entity: EntityId,
}

enum PreprocReferencesTarget {
    GraphMacroParams(Vec<GraphEntityTarget>),
    GraphMacros(Vec<FileRange>),
}

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
    let target = dispatch_references_target(db, file_id, offset, parsed_file.root())?;
    render_references_target(db, file_id, &sema, target, config)
}

fn dispatch_references_target<'tree>(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    root: Option<SyntaxNode<'tree>>,
) -> Option<ReferencesTarget<'tree>> {
    if let Some(target) = dispatch_source_graph_references_target(db, file_id, offset) {
        return Some(target);
    }
    let root = root?;
    let target =
        generated_syntax_target_at_offset(db, file_id, root, offset, SourcePurpose::FindReferences)
            .or_else(|| syntax_target_at_offset(root, offset, token_precedence))?;
    Some(ReferencesTarget::Source(target))
}

fn dispatch_source_graph_references_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<ReferencesTarget<'static>> {
    let target = PositionResolver::new(db).resolve_position(
        SourceFilePosition { file_id, offset },
        SourcePurpose::FindReferences,
        None,
    );
    let GraphSourceTargetResolution::Resolved(target) = target else {
        return None;
    };

    match target.target {
        GraphSourceTarget::MacroParamDefinition(_) => {
            Some(ReferencesTarget::Preproc(PreprocReferencesTarget::GraphMacroParams(vec![
                GraphEntityTarget { model_file_id: target.model_file_id, entity: target.entity },
            ])))
        }
        GraphSourceTarget::MacroParamReference(_) => {
            dispatch_graph_macro_param_reference_references_target(db, file_id, target)
                .map(ReferencesTarget::Preproc)
        }
        GraphSourceTarget::MacroDefinition(_) => {
            let source_graph = db.source_graph_preproc_model(target.model_file_id);
            let source_graph = source_graph.as_ref().as_ref().ok()?;
            let SourceRangeResult::Mapped(range) = source_graph
                .graph
                .entity_focus_file_range(target.entity, SourcePurpose::FindReferences)
            else {
                return None;
            };
            Some(ReferencesTarget::Preproc(PreprocReferencesTarget::GraphMacros(vec![range])))
        }
        GraphSourceTarget::MacroReference(_) => {
            dispatch_graph_macro_reference_references_target(db, file_id, target)
                .map(ReferencesTarget::Preproc)
        }
        GraphSourceTarget::Include(_)
        | GraphSourceTarget::MacroCall(_)
        | GraphSourceTarget::ExpansionToken(_)
        | GraphSourceTarget::HirSymbol(_)
        | GraphSourceTarget::HirReference(_)
        | GraphSourceTarget::SyntaxToken(_) => None,
    }
}

fn render_references_target(
    db: &RootDb,
    file_id: FileId,
    sema: &Semantics<RootDb>,
    target: ReferencesTarget<'_>,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    match target {
        ReferencesTarget::Preproc(target) => {
            render_preproc_references_target(db, file_id, target, &config)
        }
        ReferencesTarget::Source(target) => {
            render_source_references_target(sema, file_id, target, config)
        }
    }
}

fn render_source_references_target(
    sema: &Semantics<RootDb>,
    file_id: FileId,
    target: SyntaxTarget<'_>,
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

fn dispatch_graph_macro_reference_references_target(
    db: &RootDb,
    _file_id: FileId,
    reference: ResolvedSourceTarget,
) -> Option<PreprocReferencesTarget> {
    let source_graph = db.source_graph_preproc_model(reference.model_file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let definitions = source_graph
        .graph
        .resolved_definitions(source_graph.root_context, reference.entity)
        .iter()
        .filter_map(|(definition, _)| {
            let SourceRangeResult::Mapped(range) = source_graph
                .graph
                .entity_focus_file_range(*definition, SourcePurpose::FindReferences)
            else {
                return None;
            };
            Some(range)
        })
        .collect_vec();
    (!definitions.is_empty()).then_some(PreprocReferencesTarget::GraphMacros(definitions))
}

fn dispatch_graph_macro_param_reference_references_target(
    db: &RootDb,
    _file_id: FileId,
    reference: ResolvedSourceTarget,
) -> Option<PreprocReferencesTarget> {
    let source_graph = db.source_graph_preproc_model(reference.model_file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let definitions = source_graph
        .graph
        .resolved_definitions(source_graph.root_context, reference.entity)
        .iter()
        .map(|(definition, _)| GraphEntityTarget {
            model_file_id: reference.model_file_id,
            entity: *definition,
        })
        .collect_vec();
    (!definitions.is_empty()).then_some(PreprocReferencesTarget::GraphMacroParams(definitions))
}

fn render_preproc_references_target(
    db: &RootDb,
    file_id: FileId,
    target: PreprocReferencesTarget,
    config: &ReferencesConfig,
) -> Option<Vec<References>> {
    match target {
        PreprocReferencesTarget::GraphMacroParams(definitions) => definitions
            .into_iter()
            .map(|definition| graph_macro_param_references_for_definition(db, definition, config))
            .collect(),
        PreprocReferencesTarget::GraphMacros(definitions) => definitions
            .into_iter()
            .map(|definition| {
                graph_macro_references_for_definition(db, file_id, definition, config)
            })
            .collect(),
    }
}

fn graph_macro_param_references_for_definition(
    db: &RootDb,
    definition: GraphEntityTarget,
    config: &ReferencesConfig,
) -> Option<References> {
    let source_graph = db.source_graph_preproc_model(definition.model_file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let mut refs = Vec::new();
    for (reference, _) in graph.resolved_references(source_graph.root_context, definition.entity) {
        let SourceEntity::MacroParamReference(_) = graph.entity(*reference) else {
            continue;
        };
        let SourceRangeResult::Mapped(file_range) =
            graph.entity_focus_file_range(*reference, SourcePurpose::FindReferences)
        else {
            continue;
        };
        if config.search_scope.as_ref().is_none_or(|scope| {
            scope.range_for_file(file_range.file_id).is_some_and(|range| {
                range.is_none_or(|range| range.intersect(file_range.range).is_some())
            })
        }) {
            refs.push(file_range);
        }
    }
    let refs = refs
        .into_iter()
        .into_group_map_by(|usage| usage.file_id)
        .into_iter()
        .map(|(file_id, usages)| {
            (
                file_id,
                usages
                    .into_iter()
                    .map(|usage| (usage.range, ReferenceCategory::empty()))
                    .collect_vec(),
            )
        })
        .collect();
    Some(References {
        def: Some(vec![graph_macro_param_nav_target(db, graph, definition.entity)?]),
        refs,
        status: ReferencesStatus::Complete,
    })
}

fn graph_macro_references_for_definition(
    db: &RootDb,
    file_id: FileId,
    definition_range: FileRange,
    config: &ReferencesConfig,
) -> Option<References> {
    let refs = graph_macro_reference_ranges_for_definition(db, file_id, definition_range, config);
    let status = graph_preproc_reference_status(db, definition_range.file_id);
    let (nav_source_graph, nav_definition) =
        find_graph_macro_definition_model_by_focus_range(db, file_id, definition_range)?;
    let refs = refs
        .into_iter()
        .into_group_map_by(|usage| usage.file_id)
        .into_iter()
        .map(|(file_id, usages)| {
            (
                file_id,
                usages
                    .into_iter()
                    .map(|usage| (usage.range, ReferenceCategory::empty()))
                    .collect_vec(),
            )
        })
        .collect();
    Some(References {
        def: Some(vec![graph_macro_nav_target(db, &nav_source_graph.graph, nav_definition)?]),
        refs,
        status,
    })
}

fn graph_macro_reference_ranges_for_definition(
    db: &RootDb,
    file_id: FileId,
    definition_range: FileRange,
    config: &ReferencesConfig,
) -> Vec<FileRange> {
    let mut refs = Vec::new();
    for source_graph in graph_preproc_reference_models(db, file_id) {
        let Some(definition) =
            find_graph_macro_definition_by_focus_range(&source_graph.graph, definition_range)
        else {
            continue;
        };
        for (reference, _) in
            source_graph.graph.resolved_references(source_graph.root_context, definition)
        {
            let SourceEntity::MacroReference(_) = source_graph.graph.entity(*reference) else {
                continue;
            };
            let SourceRangeResult::Mapped(file_range) = source_graph
                .graph
                .entity_focus_file_range(*reference, SourcePurpose::FindReferences)
            else {
                continue;
            };
            if config.search_scope.as_ref().is_none_or(|scope| {
                scope.range_for_file(file_range.file_id).is_some_and(|range| {
                    range.is_none_or(|range| range.intersect(file_range.range).is_some())
                })
            }) {
                refs.push(file_range);
            }
        }
    }
    refs
}

fn graph_preproc_reference_models(
    db: &RootDb,
    file_id: FileId,
) -> Vec<hir::base_db::source_db::SourceGraphPreprocModel> {
    source_graph_model_file_ids_for_file(db, file_id)
        .into_iter()
        .filter_map(|model_file_id| match db.source_graph_preproc_model(model_file_id).as_ref() {
            Ok(source_graph) => Some(source_graph.clone()),
            Err(_) => None,
        })
        .collect()
}

fn graph_preproc_reference_status(db: &RootDb, file_id: FileId) -> ReferencesStatus {
    match db.source_preproc_contexts_for_file(file_id).status {
        SourcePreprocContextStatus::Complete => ReferencesStatus::Complete,
        SourcePreprocContextStatus::Partial { skipped_models } => ReferencesStatus::Partial {
            reason: ReferencesPartialReason::PreprocMacroIndex,
            issue_count: skipped_models,
        },
    }
}

fn find_graph_macro_definition_by_focus_range(
    graph: &SourceGraph,
    definition_range: FileRange,
) -> Option<EntityId> {
    graph
        .entities_intersecting_file_range(definition_range.file_id, definition_range.range, None)
        .into_iter()
        .find_map(|hit| {
            let SourceEntity::MacroDefinition(_) = graph.entity(hit.entity) else {
                return None;
            };
            let SourceRangeResult::Mapped(range) =
                graph.entity_focus_file_range(hit.entity, SourcePurpose::FindReferences)
            else {
                return None;
            };
            (range == definition_range).then_some(hit.entity)
        })
}

fn find_graph_macro_definition_model_by_focus_range(
    db: &RootDb,
    file_id: FileId,
    definition_range: FileRange,
) -> Option<(hir::base_db::source_db::SourceGraphPreprocModel, EntityId)> {
    graph_preproc_reference_models(db, file_id).into_iter().find_map(|source_graph| {
        let definition =
            find_graph_macro_definition_by_focus_range(&source_graph.graph, definition_range)?;
        Some((source_graph, definition))
    })
}

fn graph_macro_param_nav_target(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    entity: EntityId,
) -> Option<NavTarget> {
    let SourceRangeResult::Mapped(file_range) =
        graph.entity_focus_file_range(entity, SourcePurpose::FindReferences)
    else {
        return None;
    };
    let text = db.file_text(file_range.file_id);
    let name = text[file_range.range].to_owned();
    let container_name = graph.entity_parents(entity).iter().find_map(|parent| {
        let SourceEntity::MacroDefinition(_) = graph.entity(*parent) else {
            return None;
        };
        let SourceRangeResult::Mapped(parent_range) =
            graph.entity_focus_file_range(*parent, SourcePurpose::FindReferences)
        else {
            return None;
        };
        let text = db.file_text(parent_range.file_id);
        Some(text[parent_range.range].to_owned().into())
    });
    Some(NavTarget {
        file_id: file_range.file_id,
        full_range: file_range.range,
        focus_range: Some(file_range.range),
        name: Some(name.into()),
        kind: None,
        container_name,
        description: Some("macro parameter".to_owned()),
    })
}

fn graph_macro_nav_target(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    entity: EntityId,
) -> Option<NavTarget> {
    let SourceRangeResult::Mapped(file_range) =
        graph.entity_focus_file_range(entity, SourcePurpose::FindReferences)
    else {
        return None;
    };
    let text = db.file_text(file_range.file_id);
    let name = text[file_range.range].to_owned();
    Some(NavTarget {
        file_id: file_range.file_id,
        full_range: file_range.range,
        focus_range: Some(file_range.range),
        name: Some(name.into()),
        kind: None,
        container_name: None,
        description: Some("macro definition".to_owned()),
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

fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}
