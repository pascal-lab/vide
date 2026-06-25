use hir::{file::HirFileId, semantics::Semantics};
use itertools::Itertools;
use utils::line_index::covering_range;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    facts::{
        SemanticFacts, TargetQuery,
        target::{SemanticTarget, TargetIntent},
    },
    navigation_target::{NavTarget, ToNav},
    source_targets::SourceTarget,
};

pub(crate) fn goto_declaration(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let target = SemanticFacts::new(db).target_at(TargetQuery {
        file_id,
        offset,
        intent: TargetIntent::Navigate,
        root: parsed_file.root(),
    });
    render_declaration_target(
        db,
        hir_file_id,
        &sema,
        target.targets_for_intent(TargetIntent::Navigate),
    )
}

fn render_declaration_target(
    db: &RootDb,
    hir_file_id: HirFileId,
    sema: &Semantics<RootDb>,
    targets: Vec<SemanticTarget<'_>>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let mut ranges = Vec::new();
    let mut navs = Vec::new();
    for target in targets {
        let SemanticTarget::Source(target) = target else {
            return None;
        };
        let target = render_source_declaration_target(db, hir_file_id, sema, target)?;
        ranges.push(target.range);
        navs.extend(target.info);
    }

    let range = covering_range(&ranges)?;
    Some(RangeInfo::new(range, navs.into_iter().unique().collect()))
}

fn render_source_declaration_target(
    db: &RootDb,
    hir_file_id: HirFileId,
    sema: &Semantics<RootDb>,
    target: SourceTarget<'_>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let (range, tokens) = target.into_parts();

    let origins = tokens
        .into_iter()
        .filter_map(|token| match DefinitionClass::resolve(sema, hir_file_id, token)? {
            DefinitionClass::Definition(definition) => {
                Some(definition.declaration_origins().into_iter().collect_vec())
            }
            DefinitionClass::PortConnShorthand { port, .. } => {
                Some(port.declaration_origins().into_iter().collect_vec())
            }
            DefinitionClass::Ambiguous(definitions) => Some(
                definitions
                    .into_iter()
                    .filter_map(|definition| definition.declaration_origins())
                    .collect_vec(),
            ),
        })
        .flatten()
        .collect_vec();

    let navs = origins.into_iter().unique().filter_map(|def| def.to_nav(db)).collect_vec();

    Some(RangeInfo::new(range, navs))
}
