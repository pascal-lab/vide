use utils::line_index::TextSize;
use vfs::FileId;

use crate::{RangeInfo, db::root_db::RootDb, markup::Markup, semantic_target::PreprocMacroTarget};

mod expansion;
mod markup;

#[cfg(test)]
pub(super) use expansion::macro_expansion_hover_text;
pub(super) use expansion::with_expanded_macro_hover;

pub(super) fn render_macro_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    target: PreprocMacroTarget,
) -> Option<RangeInfo<Markup>> {
    match target {
        PreprocMacroTarget::ParamDefinition(definition) => Some(RangeInfo::new(
            definition.range,
            markup::macro_param_definition_markup(&definition),
        )),
        PreprocMacroTarget::ParamReference(param_resolution) => Some(RangeInfo::new(
            param_resolution.range,
            markup::macro_param_definitions_markup(&param_resolution.definitions),
        )),
        PreprocMacroTarget::Definition(definition) => Some(RangeInfo::new(
            definition.name_range,
            markup::macro_definition_markup(db, file_id, &definition),
        )),
        PreprocMacroTarget::Reference(resolution) => {
            if resolution.definitions.is_empty() {
                return expansion::expanded_macro_hover(db, file_id, offset, Some(&resolution));
            }
            expansion::expanded_macro_hover(db, file_id, offset, Some(&resolution)).or_else(|| {
                Some(RangeInfo::new(
                    resolution.range,
                    markup::macro_definitions_markup(db, file_id, &resolution.definitions),
                ))
            })
        }
    }
}
