use hir::semantics::Semantics;
use itertools::Itertools;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    goto_definition,
    navigation_target::{NavTarget, ToNav},
};

pub(crate) fn goto_declaration(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root()?;
    let selection = crate::source_tokens::source_token_resolution_at_offset(
        db,
        file_id,
        root,
        offset,
        goto_definition::token_precedence,
    )?
    .resolved()?;
    let (range, tokens) = selection.into_parts();

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
