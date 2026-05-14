use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use itertools::Itertools;
use span::{FilePosition, RangeInfo};
use syntax::{SyntaxNodeExt, has_text_range::HasTextRange};

use crate::{
    definitions::DefinitionClass,
    goto_definition,
    navigation_target::{NavTarget, ToNav},
};

pub(crate) fn goto_declaration(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let root = sema.parse_root(file_id);
    let token = root.token_at_offset(offset).pick_bext_token(goto_definition::token_precedence)?;

    let origins = match DefinitionClass::resolve(&sema, token)? {
        DefinitionClass::Definition(definition) => definition.declaration_origins(),
        DefinitionClass::PortConnShorthand { port, .. } => port.declaration_origins(),
    };

    let navs = origins.into_iter().unique().map(|def| def.to_nav(db)).collect_vec();

    Some(RangeInfo::new(token.text_range()?, navs))
}
