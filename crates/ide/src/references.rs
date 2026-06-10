use hir::{
    file::HirFileId,
    preproc::{
        MacroDefinition, MacroParamDefinition, MacroReferenceIndexStatus, macro_definition_at,
        macro_param_definition_at, macro_param_reference_definitions_at, macro_param_references,
        macro_reference_definitions_at, macro_references,
    },
    semantics::Semantics,
    source_resolver::PositionResolver,
};
use itertools::Itertools;
use nohash_hasher::IntMap;
use search::{ReferencesCtx, SearchScope};
use source_model::{
    FilePosition as SourceFilePosition, SourcePurpose, SourceTarget as GraphSourceTarget,
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
    source_targets::{SourceTarget, source_target_at_offset},
};

pub(crate) mod search;

enum ReferencesTarget<'tree> {
    Preproc(PreprocReferencesTarget),
    Source(SourceTarget<'tree>),
}

enum PreprocReferencesTarget {
    MacroParams(Vec<MacroParamDefinition>),
    Macros(Vec<MacroDefinition>),
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
        source_target_at_offset(db, file_id, root, offset, token_precedence)?.resolved()?;
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

    match target {
        GraphSourceTarget::MacroParamDefinition(_) => {
            dispatch_macro_param_definition_references_target(db, file_id, offset)
                .map(ReferencesTarget::Preproc)
        }
        GraphSourceTarget::MacroParamReference(_) => {
            dispatch_macro_param_reference_references_target(db, file_id, offset)
                .map(ReferencesTarget::Preproc)
        }
        GraphSourceTarget::MacroDefinition(_) => {
            dispatch_macro_definition_references_target(db, file_id, offset)
                .map(ReferencesTarget::Preproc)
        }
        GraphSourceTarget::MacroReference(_) => {
            dispatch_macro_reference_references_target(db, file_id, offset)
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

fn dispatch_macro_definition_references_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocReferencesTarget> {
    macro_definition_at(db, file_id, offset)
        .ok()?
        .map(|definition| PreprocReferencesTarget::Macros(vec![definition]))
}

fn dispatch_macro_reference_references_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocReferencesTarget> {
    let definitions = macro_reference_definitions_at(db, file_id, offset).ok()??.definitions;
    (!definitions.is_empty()).then_some(PreprocReferencesTarget::Macros(definitions))
}

fn dispatch_macro_param_definition_references_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocReferencesTarget> {
    macro_param_definition_at(db, file_id, offset)
        .ok()?
        .map(|definition| PreprocReferencesTarget::MacroParams(vec![definition]))
}

fn dispatch_macro_param_reference_references_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocReferencesTarget> {
    let definitions = macro_param_reference_definitions_at(db, file_id, offset).ok()??.definitions;
    (!definitions.is_empty()).then_some(PreprocReferencesTarget::MacroParams(definitions))
}

fn render_preproc_references_target(
    db: &RootDb,
    file_id: FileId,
    target: PreprocReferencesTarget,
    config: &ReferencesConfig,
) -> Option<Vec<References>> {
    match target {
        PreprocReferencesTarget::MacroParams(definitions) => definitions
            .into_iter()
            .map(|definition| {
                macro_param_references_for_definition(db, file_id, definition, config)
            })
            .collect(),
        PreprocReferencesTarget::Macros(definitions) => definitions
            .into_iter()
            .map(|definition| macro_references_for_definition(db, file_id, definition, config))
            .collect(),
    }
}

fn macro_param_references_for_definition(
    db: &RootDb,
    file_id: FileId,
    definition: MacroParamDefinition,
    config: &ReferencesConfig,
) -> Option<References> {
    let refs = macro_param_references(db, file_id, &definition)
        .ok()?
        .references
        .into_iter()
        .filter(|usage| {
            config.search_scope.as_ref().is_none_or(|scope| {
                scope.range_for_file(usage.file_id).is_some_and(|range| {
                    range.is_none_or(|range| range.intersect(usage.range).is_some())
                })
            })
        })
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
        def: Some(vec![macro_param_nav_target(definition)]),
        refs,
        status: ReferencesStatus::Complete,
    })
}

fn macro_references_for_definition(
    db: &RootDb,
    file_id: FileId,
    definition: MacroDefinition,
    config: &ReferencesConfig,
) -> Option<References> {
    let references = macro_references(db, file_id, &definition).ok()?;
    let status = references_status_from_macro_index(references.status);
    let refs = references
        .references
        .into_iter()
        .filter(|usage| {
            config.search_scope.as_ref().is_none_or(|scope| {
                scope.range_for_file(usage.file_id).is_some_and(|range| {
                    range.is_none_or(|range| range.intersect(usage.range).is_some())
                })
            })
        })
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
    Some(References { def: Some(vec![macro_nav_target(definition)]), refs, status })
}

fn references_status_from_macro_index(status: MacroReferenceIndexStatus) -> ReferencesStatus {
    match status {
        MacroReferenceIndexStatus::Complete => ReferencesStatus::Complete,
        MacroReferenceIndexStatus::Partial { issues } => ReferencesStatus::Partial {
            reason: ReferencesPartialReason::PreprocMacroIndex,
            issue_count: issues.len(),
        },
    }
}

fn macro_param_nav_target(definition: MacroParamDefinition) -> NavTarget {
    NavTarget {
        file_id: definition.macro_definition.file_id,
        full_range: definition.range,
        focus_range: Some(definition.range),
        name: Some(definition.name),
        kind: None,
        container_name: Some(definition.macro_definition.name),
        description: Some("macro parameter".to_owned()),
    }
}

fn macro_nav_target(definition: MacroDefinition) -> NavTarget {
    NavTarget {
        file_id: definition.file_id,
        full_range: definition.name_range,
        focus_range: Some(definition.name_range),
        name: Some(definition.name),
        kind: None,
        container_name: None,
        description: Some("macro definition".to_owned()),
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

#[cfg(test)]
mod tests {
    use hir::preproc::{MacroReferenceIndexIssue, PreprocError};

    use super::*;

    #[test]
    fn macro_reference_index_status_maps_to_reference_status() {
        assert_eq!(
            references_status_from_macro_index(MacroReferenceIndexStatus::Complete),
            ReferencesStatus::Complete
        );

        let status = references_status_from_macro_index(MacroReferenceIndexStatus::Partial {
            issues: vec![MacroReferenceIndexIssue::SkippedModel {
                file_id: FileId(0),
                error: PreprocError::MissingRootSource,
            }],
        });

        assert_eq!(
            status,
            ReferencesStatus::Partial {
                reason: ReferencesPartialReason::PreprocMacroIndex,
                issue_count: 1,
            }
        );
        assert!(status.is_partial());
        assert_eq!(status.issue_count(), 1);
    }
}
