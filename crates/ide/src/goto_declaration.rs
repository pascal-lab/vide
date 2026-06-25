use hir::semantics::Semantics;
use itertools::Itertools;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    goto_definition,
    navigation_target::{NavTarget, ToNav},
    semantic_target::{SemanticTarget, TargetIntent, resolve_semantic_target},
};

pub(crate) fn goto_declaration(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let target = resolve_semantic_target(
        db,
        file_id,
        offset,
        parsed_file.root(),
        goto_definition::token_precedence,
    );
    let SemanticTarget::Source(target) = target.unique_for_intent(TargetIntent::Navigate)? else {
        return None;
    };
    let (range, tokens) = target.into_parts();

    let origins = tokens
        .into_iter()
        .filter_map(|token| match DefinitionClass::resolve(&sema, hir_file_id, token)? {
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
