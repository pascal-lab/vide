use hir::{semantics::Semantics, source_resolver::PositionResolver};
use itertools::Itertools;
use source_model::{
    FilePosition as SourceFilePosition, ResolvedSourceTarget, SourcePurpose,
    SourceTarget as GraphSourceTarget, SourceTargetResolution as GraphSourceTargetResolution,
};
use syntax::{SyntaxNodeExt, has_text_range::HasTextRange};
use vfs::FileId;

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
    if !source_graph_allows_goto_declaration(db, file_id, offset) {
        return None;
    }
    let token = root
        .token_at_offset(offset)
        .pick_bext_token(|kind| (goto_definition::token_precedence(kind) > 1).into())?;
    let range = token.text_range()?;

    let origins = [token]
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

fn source_graph_allows_goto_declaration(
    db: &RootDb,
    file_id: FileId,
    offset: utils::line_index::TextSize,
) -> bool {
    let target = PositionResolver::new(db).resolve_position(
        SourceFilePosition { file_id, offset },
        SourcePurpose::GotoDefinition,
        None,
    );

    matches!(
        target,
        GraphSourceTargetResolution::None
            | GraphSourceTargetResolution::Resolved(ResolvedSourceTarget {
                target: GraphSourceTarget::MacroCall(_)
                    | GraphSourceTarget::HirSymbol(_)
                    | GraphSourceTarget::HirReference(_)
                    | GraphSourceTarget::SyntaxToken(_),
                ..
            })
    )
}
