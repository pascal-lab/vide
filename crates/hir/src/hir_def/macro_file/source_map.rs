use ::preproc::source::{PreprocSourceId, SourceEmittedTokenRange, SourceRange};
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SourceBufferRange,
    preproc::{MacroCallId, MacroDefinitionId, TokenOrigin, Trace},
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::base_db::source_db::PreprocSourceMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    File { file: FileId, range: TextRange },
    MacroBody { call: MacroCallId, def: MacroDefinitionId, body_range: TextRange },
    MacroArg { call: MacroCallId, arg_index: usize, arg_range: TextRange },
    TokenPaste { call: MacroCallId },
    Stringify { call: MacroCallId },
    Builtin { call: MacroCallId, name: SmolStr },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExpansionSourceMap {
    origins: Vec<Option<Origin>>,
}

impl ExpansionSourceMap {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }

    pub fn map_up(&self, expanded_token_index: usize) -> Option<Origin> {
        self.origins.get(expanded_token_index).cloned().flatten()
    }

    pub fn map_down(&self, origin: &Origin) -> Vec<usize> {
        self.origins
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| (candidate.as_ref() == Some(origin)).then_some(index))
            .collect()
    }

    pub(crate) fn from_trace_range(
        trace: &Trace,
        source_map: &PreprocSourceMap,
        emitted_range: SourceEmittedTokenRange,
    ) -> Self {
        let Some(end) = emitted_range.start.raw().checked_add(emitted_range.len) else {
            return Self::empty();
        };
        let origins = (emitted_range.start.raw()..end)
            .map(|raw| {
                trace
                    .emitted_tokens
                    .get(raw)
                    .and_then(|token| origin_from_token_origin(&token.provenance, source_map))
            })
            .collect();
        Self { origins }
    }

    #[cfg(test)]
    pub(crate) fn from_token_origins<'a>(
        origins: impl IntoIterator<Item = &'a TokenOrigin>,
        source_map: &PreprocSourceMap,
    ) -> Self {
        Self {
            origins: origins
                .into_iter()
                .map(|origin| origin_from_token_origin(origin, source_map))
                .collect(),
        }
    }
}

fn origin_from_token_origin(origin: &TokenOrigin, source_map: &PreprocSourceMap) -> Option<Origin> {
    match origin {
        TokenOrigin::Source { token_range } => file_origin(source_map, token_range),
        TokenOrigin::MacroBody { identity, body_token_range, .. } => Some(Origin::MacroBody {
            call: identity.call_id,
            def: identity.definition_id,
            body_range: text_range(body_token_range)?,
        }),
        TokenOrigin::MacroArgument { identity, argument_token_range, .. } => {
            Some(Origin::MacroArg {
                call: identity.call_id,
                arg_index: usize::try_from(identity.argument_index).ok()?,
                arg_range: text_range(argument_token_range)?,
            })
        }
        TokenOrigin::TokenPaste { identity } => Some(Origin::TokenPaste { call: identity.call_id }),
        TokenOrigin::Stringification { identity } => {
            Some(Origin::Stringify { call: identity.call_id })
        }
        TokenOrigin::Builtin { name, identity } if !name.is_empty() => {
            Some(Origin::Builtin { call: identity.call_id, name: name.to_smolstr() })
        }
        TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => None,
    }
}

fn file_origin(source_map: &PreprocSourceMap, token_range: &SourceBufferRange) -> Option<Origin> {
    let source_range = source_range_from_trace(token_range)?;
    let range = source_map.map_range(source_range).ok()?;
    let file = source_map.file_id(source_range.source).ok()?;
    Some(Origin::File { file, range })
}

fn source_range_from_trace(range: &SourceBufferRange) -> Option<SourceRange> {
    Some(SourceRange { source: PreprocSourceId::from(range.buffer_id), range: text_range(range)? })
}

fn text_range(range: &SourceBufferRange) -> Option<TextRange> {
    Some(TextRange::new(
        TextSize::from(u32::try_from(range.range.start).ok()?),
        TextSize::from(u32::try_from(range.range.end).ok()?),
    ))
}
