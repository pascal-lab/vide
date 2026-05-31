use hir::semantics::Semantics;
use itertools::Itertools;
use syntax::{SyntaxNodeExt, has_text_range::HasTextRange};

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
    let token = root.token_at_offset(offset).pick_bext_token(goto_definition::token_precedence)?;

    let origins = match DefinitionClass::resolve(&sema, hir_file_id, token)? {
        DefinitionClass::Definition(definition) => {
            definition.declaration_origins().into_iter().collect_vec()
        }
        DefinitionClass::PortConnShorthand { port, .. } => {
            port.declaration_origins().into_iter().collect_vec()
        }
        DefinitionClass::Ambiguous(definitions) => definitions
            .into_iter()
            .filter_map(|definition| definition.declaration_origins())
            .collect_vec(),
    };

    let navs = origins.into_iter().unique().filter_map(|def| def.to_nav(db)).collect_vec();

    Some(RangeInfo::new(token.text_range()?, navs))
}
