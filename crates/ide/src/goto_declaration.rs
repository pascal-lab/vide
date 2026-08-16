use itertools::Itertools;
use preproc_expand::file::HirFileId;
use utils::line_index::covering_range;

use crate::{
    FilePosition, RangeInfo,
    analysis::AnalysisContext,
    definitions::DefinitionClass,
    navigation_target::{NavTarget, ToNav},
    semantic_target::{SemanticTarget, SourceTarget, TargetIntent, resolve_semantic_target},
};

pub(crate) fn goto_declaration(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = db.semantics();
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let target = resolve_semantic_target(
        db.db,
        file_id,
        offset,
        parsed_file.root(),
        crate::token::navigation_precedence,
    );
    render_declaration_target(db, hir_file_id, target.targets_for_intent(TargetIntent::Navigate))
}

fn render_declaration_target(
    db: &AnalysisContext<'_>,
    hir_file_id: HirFileId,
    targets: Vec<SemanticTarget<'_>>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let mut ranges = Vec::new();
    let mut navs = Vec::new();
    for target in targets {
        let target = match target {
            SemanticTarget::Manifest(target) => crate::manifest::definition_target(db, target),
            SemanticTarget::Source(target) => {
                render_source_declaration_target(db, hir_file_id, target)
            }
            SemanticTarget::PreprocMacro(_) | SemanticTarget::Include(_) => None,
        };
        let target = target?;
        ranges.push(target.range);
        navs.extend(target.info);
    }

    let range = covering_range(&ranges)?;
    Some(RangeInfo::new(range, navs.into_iter().unique().collect()))
}

fn render_source_declaration_target(
    db: &AnalysisContext<'_>,
    hir_file_id: HirFileId,
    target: SourceTarget<'_>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let (range, tokens) = target.into_parts();

    let origins = tokens
        .into_iter()
        .flat_map(|token| {
            DefinitionClass::resolve(db, hir_file_id, token).into_candidates().into_iter().map(
                |class| match class {
                    DefinitionClass::Definition(definition) => definition.declaration_origin(db.db),
                    DefinitionClass::PortConnShorthand { port, .. } => {
                        port.declaration_origin(db.db)
                    }
                },
            )
        })
        .collect_vec();

    let navs = origins.into_iter().unique().filter_map(|def| def.to_nav(db)).collect_vec();
    (!navs.is_empty()).then_some(RangeInfo::new(range, navs))
}
