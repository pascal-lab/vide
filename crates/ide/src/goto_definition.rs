use hir::{
    base_db::source_db::SourceDb,
    container::InFile,
    file::HirFileId,
    preproc::{IncludeDirective, IncludeTarget, MacroDefinition, MacroParamDefinition},
    semantics::Semantics,
};
use itertools::Itertools;
use syntax::{SyntaxTokenWithParent, token::pair_token};
#[cfg(test)]
use syntax::{TokenKind, token::TokenKindExt};
use utils::line_index::{TextRange, TextSize, covering_range};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    facts::{
        SemanticFacts, TargetQuery,
        target::{
            PreprocMacroTarget, SemanticTarget, SourceTarget, TargetIntent, TargetResolution,
        },
    },
    navigation_target::{NavTarget, ToNav},
};

pub(crate) fn goto_definition(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let target = SemanticFacts::new(db).target_at(TargetQuery {
        file_id,
        offset,
        intent: TargetIntent::Navigate,
        root: parsed_file.root(),
    });
    render_definition_target(db, file_id, &sema, target)
}

fn render_definition_target(
    db: &RootDb,
    file_id: FileId,
    sema: &Semantics<RootDb>,
    target: TargetResolution<'_>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let mut ranges = Vec::new();
    let mut navs = Vec::new();
    for target in target.targets_for_intent(TargetIntent::Navigate) {
        let target = match target {
            SemanticTarget::PreprocMacro(target) => render_preproc_definition_target(target),
            SemanticTarget::Include(includes) => render_include_definition_target(db, includes),
            SemanticTarget::Source(target) => {
                render_source_definition_target(db, file_id, sema, target)
            }
        }?;
        ranges.push(target.range);
        navs.extend(target.info);
    }

    if navs.is_empty() {
        return None;
    }

    let range = covering_range(&ranges)?;
    Some(RangeInfo::new(range, navs.into_iter().unique().collect()))
}

fn render_source_definition_target(
    db: &RootDb,
    file_id: FileId,
    sema: &Semantics<RootDb>,
    target: SourceTarget<'_>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let hir_file_id = file_id.into();
    let (range, tokens) = target.into_parts();
    let navs = tokens
        .into_iter()
        .filter_map(|token| nav_targets_for_token(db, sema, hir_file_id, token))
        .flatten()
        .unique()
        .collect_vec();
    if navs.is_empty() {
        return None;
    }

    Some(RangeInfo::new(range, navs))
}

fn nav_targets_for_token(
    db: &RootDb,
    sema: &Semantics<RootDb>,
    hir_file_id: HirFileId,
    token: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    handle_ctrl_flow_kw(sema, hir_file_id, token).or_else(|| {
        DefinitionClass::resolve(sema, hir_file_id, token)?
            .origins()
            .into_iter()
            .unique()
            .filter_map(|def| def.to_nav(db))
            .collect_vec()
            .into()
    })
}

fn render_preproc_definition_target(
    target: PreprocMacroTarget,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    match target {
        PreprocMacroTarget::ParamDefinition(definition) => {
            Some(RangeInfo::new(definition.range, vec![macro_param_nav_target(definition)]))
        }
        PreprocMacroTarget::ParamReference(resolution) => {
            let reference_range = resolution.range;
            let targets =
                resolution.definitions.into_iter().map(macro_param_nav_target).collect_vec();
            (!targets.is_empty()).then_some(RangeInfo::new(reference_range, targets))
        }
        PreprocMacroTarget::Definition(definition) => {
            Some(RangeInfo::new(definition.name_range, vec![macro_nav_target(definition)]))
        }
        PreprocMacroTarget::Reference(resolution) => {
            let reference_range = resolution.range;
            let targets = resolution.definitions.into_iter().map(macro_nav_target).collect_vec();
            (!targets.is_empty()).then_some(RangeInfo::new(reference_range, targets))
        }
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

fn render_include_definition_target(
    db: &RootDb,
    includes: Vec<IncludeDirective>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let range = includes.first()?.range;
    let targets = includes
        .into_iter()
        .filter_map(|include| {
            let IncludeTarget::Literal { path, resolved_file: Some(target_file_id) } =
                include.target
            else {
                return None;
            };
            let target_range = TextRange::empty(TextSize::new(0));
            Some(NavTarget {
                file_id: target_file_id,
                full_range: target_range,
                focus_range: Some(target_range),
                name: Some(path),
                kind: None,
                container_name: None,
                description: db.file_path(target_file_id).map(|path| path.to_string()),
            })
        })
        .unique()
        .collect_vec();
    if targets.is_empty() {
        return None;
    }
    Some(RangeInfo::new(range, targets))
}

fn handle_ctrl_flow_kw(
    sema: &Semantics<RootDb>,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { .. }: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    let kind = tp.kind();

    match kind {
        _ if let Some(pair) = pair_token(tp) => {
            let tok = InFile::new(file_id, pair.either(|pair| pair, |_| tp));
            Some(vec![tok.to_nav(sema.db)?])
        }
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}
