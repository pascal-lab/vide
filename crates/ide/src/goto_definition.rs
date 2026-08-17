use hir_def::container::InFile;
use itertools::Itertools;
use preproc_expand::{
    file::HirFileId,
    preproc::{IncludeDirective, IncludeTarget, MacroDefinition, MacroParamDefinition},
};
use syntax::SyntaxTokenWithParent;
use utils::line_index::{TextRange, TextSize, covering_range};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo,
    analysis::AnalysisContext,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    navigation_target::{NavTarget, ToNav},
    semantic_target::{
        PreprocMacroTarget, SemanticTarget, SourceTarget, TargetIntent, TargetResolution,
        resolve_semantic_target,
    },
};

pub(crate) fn goto_definition(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    if let Some(target) = declaration_name_from_shard(db, file_id, offset) {
        return Some(target);
    }
    let tree = db.parse_file(file_id);
    let target = resolve_semantic_target(
        db.db,
        file_id,
        offset,
        Some(tree.root()),
        crate::token::navigation_precedence,
    );
    render_definition_target(db, file_id, target)
}

/// Cursor is on a compilation-unit design-unit name in this file. The
/// definition is that token; do not build the include plan or `$unit`.
fn declaration_name_from_shard(
    db: &AnalysisContext<'_>,
    file_id: FileId,
    offset: TextSize,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let decl = db.file_decl_shard(file_id).design_unit_at(offset)?.clone();
    let range = decl.name_range?;
    let kind = match decl.role {
        hir_def::decl_shard::DeclRole::Module => Some(crate::DefKind::Module),
        hir_def::decl_shard::DeclRole::Interface => Some(crate::DefKind::Interface),
        hir_def::decl_shard::DeclRole::Package => Some(crate::DefKind::Package),
        hir_def::decl_shard::DeclRole::Program => Some(crate::DefKind::Program),
        hir_def::decl_shard::DeclRole::Checker => Some(crate::DefKind::Checker),
        hir_def::decl_shard::DeclRole::Covergroup => Some(crate::DefKind::Covergroup),
        _ => None,
    };
    Some(RangeInfo::new(
        range,
        vec![NavTarget {
            file_id,
            full_range: range,
            focus_range: Some(range),
            name: Some(decl.name),
            kind,
            container_name: None,
            description: None,
        }],
    ))
}

fn render_definition_target(
    db: &AnalysisContext<'_>,
    file_id: FileId,
    target: TargetResolution<'_>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let mut ranges = Vec::new();
    let mut navs = Vec::new();
    for target in target.targets_for_intent(TargetIntent::Navigate) {
        let target = match target {
            SemanticTarget::PreprocMacro(target) => render_preproc_definition_target(target),
            SemanticTarget::Include(includes) => render_include_definition_target(db, includes),
            SemanticTarget::Manifest(target) => crate::manifest::definition_target(db, target),
            SemanticTarget::Source(target) => render_source_definition_target(db, file_id, target),
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
    db: &AnalysisContext<'_>,
    file_id: FileId,
    target: SourceTarget<'_>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let hir_file_id = file_id.into();
    let (range, tokens) = target.into_parts();
    let navs = tokens
        .into_iter()
        .filter_map(|token| nav_targets_for_token(db, hir_file_id, token))
        .flatten()
        .unique()
        .collect_vec();
    if navs.is_empty() {
        return None;
    }

    Some(RangeInfo::new(range, navs))
}

fn nav_targets_for_token(
    db: &AnalysisContext<'_>,
    hir_file_id: HirFileId,
    token: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    handle_ctrl_flow_kw(db.db, hir_file_id, token).or_else(|| {
        let navs = DefinitionClass::resolve(db, hir_file_id, token)
            .into_candidates()
            .into_iter()
            .flat_map(|class| class.origins(db.db))
            .unique()
            .filter_map(|def| def.to_nav(db.db))
            .map(compact_design_unit_target)
            .collect_vec();
        (!navs.is_empty()).then_some(navs)
    })
}

fn compact_design_unit_target(mut target: NavTarget) -> NavTarget {
    if matches!(
        target.kind,
        Some(
            crate::DefKind::Module
                | crate::DefKind::Interface
                | crate::DefKind::Program
                | crate::DefKind::Checker
                | crate::DefKind::Covergroup
        )
    ) && let Some(focus_range) = target.focus_range
    {
        target.full_range = focus_range;
    }
    target
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
    db: &RootDb,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { .. }: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    let (beg, _) = crate::token::ctrl_flow_pair(tp)?;
    let tok = InFile::new(file_id, beg);
    Some(vec![tok.to_nav(db)?])
}
