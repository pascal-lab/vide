use hir_def::container::InFile;
use itertools::Itertools;
use preproc_expand::{
    file::HirFileId,
    preproc::{IncludeDirective, IncludeTarget, MacroDefinition, MacroParamDefinition},
};
use syntax::{SyntaxAncestors, SyntaxTokenWithParent, ast::AstNode, has_text_range::HasTextRange};
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
    if let Some(target) = crate::design_unit::goto_definition(db, FilePosition { file_id, offset })
    {
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
        if !navs.is_empty() {
            return Some(navs);
        }
        slang_scoped_nav(db, hir_file_id, token)
    })
}

fn slang_scoped_nav(
    db: &AnalysisContext<'_>,
    hir_file_id: HirFileId,
    token: SyntaxTokenWithParent<'_>,
) -> Option<Vec<NavTarget>> {
    let file = hir_file_id.as_file()?;
    let scoped =
        SyntaxAncestors::start_from(token.parent).find_map(syntax::ast::ScopedName::cast)?;
    if scoped_uses_dot(scoped) {
        return None;
    }
    let range = token.text_range()?;
    let crate::elaboration::ElabResult::Ready(Some(info)) =
        crate::slang_class::lookup_symbol_at(db, file, usize::from(range.start()))
    else {
        return None;
    };
    if info.def_file.is_empty() {
        return None;
    }
    let file_id = crate::anchor::file_id_for_slang_path(db.db, &info.def_file);
    let start = utils::line_index::TextSize::from(info.def_offset as u32);
    let len = utils::line_index::TextSize::from(info.name.len() as u32);
    let focus = utils::line_index::TextRange::new(start, start + len);
    Some(vec![NavTarget {
        file_id,
        full_range: focus,
        focus_range: Some(focus),
        name: Some(smol_str::SmolStr::from(info.name.as_str())),
        kind: None,
        container_name: None,
        description: None,
    }])
}

fn scoped_uses_dot(scoped: syntax::ast::ScopedName<'_>) -> bool {
    scoped
        .syntax()
        .children()
        .filter_map(|elem| elem.as_token())
        .any(|tok| tok.kind() == syntax::Token![.])
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
