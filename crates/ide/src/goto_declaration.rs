use hir::semantics::Semantics;
use itertools::Itertools;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    goto_definition,
    navigation_target::{NavTarget, ToNav},
    source_tokens::SourceTokenSelection,
};

pub(crate) fn goto_declaration(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root()?;
    let selection = crate::source_tokens::token_candidates_at_offset(
        db,
        file_id,
        root,
        offset,
        goto_definition::token_precedence,
    )?;
    let (range, tokens) = match selection {
        SourceTokenSelection::NormalSyntax(selection) => (selection.range, selection.tokens),
        SourceTokenSelection::Preproc(selection) => {
            let _ = selection.hits.len();
            (selection.range, selection.tokens)
        }
        SourceTokenSelection::Unavailable(unavailable) => {
            let _ = unavailable.range;
            return None;
        }
        SourceTokenSelection::Ambiguous(ambiguous) => {
            let _ = (ambiguous.range, ambiguous.hits.len());
            return None;
        }
    };

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
