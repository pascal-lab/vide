use hir::{
    file::HirFileId,
    preproc::{
        MacroDefinition, MacroParamDefinition, macro_definition_at, macro_param_definition_at,
        macro_param_reference_definitions_at, macro_param_references,
        macro_reference_definitions_at, macro_references,
    },
    semantics::Semantics,
};
use itertools::Itertools;
use nohash_hasher::IntMap;
use search::{ReferencesCtx, SearchScope};
use syntax::{
    SyntaxTokenWithParent, TokenKind,
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
}

pub(crate) fn references(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    config: ReferencesConfig,
) -> Option<Vec<References>> {
    if let Some(macro_refs) = handle_preproc_macro(db, file_id, offset, &config) {
        return Some(macro_refs);
    }

    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root()?;
    let selection = crate::source_tokens::token_candidates_at_offset(
        db,
        file_id,
        root,
        offset,
        token_precedence,
    )?;
    let references = selection
        .tokens
        .into_iter()
        .filter_map(|token| references_for_token(&sema, hir_file_id, token, config.clone()))
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

fn handle_preproc_macro(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    config: &ReferencesConfig,
) -> Option<Vec<References>> {
    if let Some(param_refs) = handle_preproc_macro_param(db, file_id, offset, config) {
        return Some(param_refs);
    }

    let definitions = if let Some(definition) = macro_definition_at(db, file_id, offset).ok()? {
        vec![definition]
    } else {
        macro_reference_definitions_at(db, file_id, offset).ok()??.definitions
    };
    if definitions.is_empty() {
        return None;
    }

    definitions
        .into_iter()
        .map(|definition| macro_references_for_definition(db, file_id, definition, config))
        .collect()
}

fn handle_preproc_macro_param(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    config: &ReferencesConfig,
) -> Option<Vec<References>> {
    let definitions =
        if let Some(definition) = macro_param_definition_at(db, file_id, offset).ok()? {
            vec![definition]
        } else {
            macro_param_reference_definitions_at(db, file_id, offset).ok()??.definitions
        };
    if definitions.is_empty() {
        return None;
    }

    definitions
        .into_iter()
        .map(|definition| macro_param_references_for_definition(db, file_id, definition, config))
        .collect()
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
    Some(References { def: Some(vec![macro_param_nav_target(definition)]), refs })
}

fn macro_references_for_definition(
    db: &RootDb,
    file_id: FileId,
    definition: MacroDefinition,
    config: &ReferencesConfig,
) -> Option<References> {
    let refs = macro_references(db, file_id, &definition)
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
    Some(References { def: Some(vec![macro_nav_target(definition)]), refs })
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

    Some(vec![References { def: None, refs: IntMap::from_iter([(file_id.file_id(), refs)]) }])
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
    References { def, refs }
}

fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}
