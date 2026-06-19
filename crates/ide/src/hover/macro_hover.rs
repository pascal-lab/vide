use hir::preproc::{
    MacroDefinition, MacroParamDefinition, MacroParamReferenceDefinitions,
    MacroReferenceDefinitions, macro_definition_at, macro_param_definition_at,
    macro_param_reference_definitions_at, macro_reference_definitions_at,
};
use utils::line_index::TextSize;
use vfs::FileId;

use crate::{RangeInfo, db::root_db::RootDb, markup::Markup};

mod expansion;
mod markup;

#[cfg(test)]
pub(super) use expansion::macro_expansion_hover_text;
pub(super) use expansion::with_expanded_macro_hover;

pub(super) enum MacroHoverTarget {
    ParamDefinition(MacroParamDefinition),
    ParamReference(MacroParamReferenceDefinitions),
    Definition(MacroDefinition),
    Reference(MacroReferenceDefinitions),
}

pub(super) fn dispatch_macro_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<MacroHoverTarget> {
    if let Ok(Some(definition)) = macro_param_definition_at(db, file_id, offset) {
        return Some(MacroHoverTarget::ParamDefinition(definition));
    }

    if let Ok(Some(param_resolution)) = macro_param_reference_definitions_at(db, file_id, offset) {
        if param_resolution.definitions.is_empty() {
            return None;
        }
        return Some(MacroHoverTarget::ParamReference(param_resolution));
    }

    if let Ok(Some(definition)) = macro_definition_at(db, file_id, offset) {
        return Some(MacroHoverTarget::Definition(definition));
    }

    if let Ok(Some(resolution)) = macro_reference_definitions_at(db, file_id, offset) {
        return Some(MacroHoverTarget::Reference(resolution));
    }

    None
}

pub(super) fn render_macro_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    target: MacroHoverTarget,
) -> Option<RangeInfo<Markup>> {
    match target {
        MacroHoverTarget::ParamDefinition(definition) => Some(RangeInfo::new(
            definition.range,
            markup::macro_param_definition_markup(&definition),
        )),
        MacroHoverTarget::ParamReference(param_resolution) => Some(RangeInfo::new(
            param_resolution.range,
            markup::macro_param_definitions_markup(&param_resolution.definitions),
        )),
        MacroHoverTarget::Definition(definition) => Some(RangeInfo::new(
            definition.name_range,
            markup::macro_definition_markup(db, file_id, &definition),
        )),
        MacroHoverTarget::Reference(resolution) => {
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
