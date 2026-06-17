use syntax::{
    SourceBufferRange,
    preproc::{MacroArgumentOrigin, MacroBodyOrigin, MacroBuiltinOrigin, MacroOperationOrigin},
};
use utils::line_index::{TextRange, TextSize};

use super::*;

pub(super) fn source_range_from_origin(range: &SourceBufferRange) -> Option<SourceRange> {
    Some(SourceRange {
        source: PreprocSourceId::from(range.buffer_id),
        range: TextRange::new(
            TextSize::from(u32::try_from(range.range.start).ok()?),
            TextSize::from(u32::try_from(range.range.end).ok()?),
        ),
    })
}

pub(super) fn macro_body_identity(value: &MacroBodyOrigin) -> SourceMacroBodyIdentity {
    SourceMacroBodyIdentity {
        call: value.call_id,
        definition: value.definition_id,
        expansion: value.expansion_id,
        parent_expansion: value.parent_expansion_id,
        body_token_index: value.body_token_index as usize,
    }
}

pub(super) fn macro_argument_identity(value: &MacroArgumentOrigin) -> SourceMacroArgumentIdentity {
    SourceMacroArgumentIdentity {
        call: value.call_id,
        definition: value.definition_id,
        expansion: value.expansion_id,
        parent_expansion: value.parent_expansion_id,
        body_token_index: value.body_token_index as usize,
        argument_index: value.argument_index as usize,
        argument_token_index: value.argument_token_index as usize,
    }
}

pub(super) fn macro_builtin_identity(value: &MacroBuiltinOrigin) -> SourceMacroBuiltinIdentity {
    SourceMacroBuiltinIdentity {
        call: value.call_id,
        expansion: value.expansion_id,
        parent_expansion: value.parent_expansion_id,
    }
}

pub(super) fn macro_operation_identity(
    value: &MacroOperationOrigin,
) -> SourceMacroOperationIdentity {
    SourceMacroOperationIdentity {
        call: value.call_id,
        definition: value.definition_id,
        expansion: value.expansion_id,
        parent_expansion: value.parent_expansion_id,
        body_token_index: value.body_token_index as usize,
        argument_index: value.argument_index.map(|index| index as usize),
        argument_token_index: value.argument_token_index.map(|index| index as usize),
    }
}
