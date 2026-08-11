use std::collections::BTreeMap;

use ::preproc::source::{PreprocSourceId, SourceRange};
use rustc_hash::FxHashMap;
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SourceBufferRange,
    preproc::{
        ActualArgument, MacroCallId as TraceMacroCallId, MacroDefinitionId, TokenOrigin, Trace,
    },
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use super::{
    ExpandError, ExpandErrorKind, ExpandResult, MacroCallId, MacroCallLoc, SourceEmittedTokenId,
    SourceEmittedTokenRange,
};
use crate::source_db::{PreprocSourceMap, range_index::RangeIndex};
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
    origins: Vec<Option<OriginSlot>>,
    source_ranges: FxHashMap<FileId, RangeIndex<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OriginSlot {
    emitted_token: SourceEmittedTokenId,
    origin: Origin,
    source: Option<OriginSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OriginSource {
    file: FileId,
    range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpansionSourceMapError {
    InvalidEmittedTokenRange { start: SourceEmittedTokenId, len: usize },
    MissingTraceToken { token: SourceEmittedTokenId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionSourceHit {
    pub emitted_token: SourceEmittedTokenId,
    pub expanded_token_index: usize,
    pub range: TextRange,
    pub origin: Origin,
}

impl ExpansionSourceMap {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }

    #[cfg(test)]
    pub fn map_up(&self, expanded_token_index: usize) -> Option<Origin> {
        self.origins
            .get(expanded_token_index)
            .and_then(|slot| slot.as_ref().map(|slot| slot.origin.clone()))
    }

    #[cfg(test)]
    pub fn map_down(&self, origin: &Origin) -> Vec<usize> {
        self.origins
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                candidate.as_ref().filter(|slot| &slot.origin == origin).map(|_| index)
            })
            .collect()
    }

    pub fn source_hits(&self, file: FileId, offset: TextSize) -> Vec<ExpansionSourceHit> {
        let Some(ranges) = self.source_ranges.get(&file) else {
            return Vec::new();
        };
        let mut expanded_token_indices = ranges.ids_at(offset);
        expanded_token_indices.sort_unstable();
        expanded_token_indices
            .into_iter()
            .filter_map(|expanded_token_index| {
                let slot = self.origins.get(expanded_token_index)?.as_ref()?;
                let source = slot.source?;
                Some(ExpansionSourceHit {
                    emitted_token: slot.emitted_token,
                    expanded_token_index,
                    range: source.range,
                    origin: slot.origin.clone(),
                })
            })
            .collect()
    }

    pub(crate) fn from_trace_range(
        model_file: FileId,
        trace: &Trace,
        source_map: &PreprocSourceMap,
        emitted_range: SourceEmittedTokenRange,
    ) -> ExpandResult<Self> {
        let start = emitted_range.start.raw();
        if start > trace.emitted_tokens.len() {
            return source_map_error(
                Self::empty(),
                ExpansionSourceMapError::InvalidEmittedTokenRange {
                    start: emitted_range.start,
                    len: emitted_range.len,
                },
            );
        }
        let Some(end) = start.checked_add(emitted_range.len) else {
            return source_map_error(
                Self::empty(),
                ExpansionSourceMapError::InvalidEmittedTokenRange {
                    start: emitted_range.start,
                    len: emitted_range.len,
                },
            );
        };
        let operation_sources = OperationSourceResolver::new(trace);
        let mut origins = Vec::new();
        for raw in start..end {
            let emitted_token = SourceEmittedTokenId::new(raw);
            let Some(token) = trace.emitted_tokens.get(raw) else {
                let source_ranges = source_ranges_for(&origins);
                return source_map_error(
                    Self { origins, source_ranges },
                    ExpansionSourceMapError::MissingTraceToken { token: emitted_token },
                );
            };
            origins.push(origin_slot_from_token_origin(
                model_file,
                emitted_token,
                &token.origin,
                source_map,
                Some(&operation_sources),
            ));
        }
        let source_ranges = source_ranges_for(&origins);
        ExpandResult::ok(Self { origins, source_ranges })
    }

    #[cfg(test)]
    pub(crate) fn from_token_origins<'a>(
        model_file: FileId,
        origins: impl IntoIterator<Item = &'a TokenOrigin>,
        source_map: &PreprocSourceMap,
    ) -> Self {
        let origins: Vec<_> = origins
            .into_iter()
            .enumerate()
            .map(|origin| {
                origin_slot_from_token_origin(
                    model_file,
                    SourceEmittedTokenId::new(origin.0),
                    origin.1,
                    source_map,
                    None,
                )
            })
            .collect();
        let source_ranges = source_ranges_for(&origins);
        Self { origins, source_ranges }
    }
}
fn source_ranges_for(origins: &[Option<OriginSlot>]) -> FxHashMap<FileId, RangeIndex<usize>> {
    let mut source_ranges = FxHashMap::default();
    for (expanded_token_index, slot) in origins.iter().enumerate() {
        let Some(source) = slot.as_ref().and_then(|slot| slot.source) else {
            continue;
        };
        source_ranges
            .entry(source.file)
            .or_insert_with(RangeIndex::default)
            .push(source.range, expanded_token_index);
    }
    for ranges in source_ranges.values_mut() {
        ranges.finish();
    }
    source_ranges
}
fn source_map_error(
    value: ExpansionSourceMap,
    error: ExpansionSourceMapError,
) -> ExpandResult<ExpansionSourceMap> {
    ExpandResult::new(value, ExpandError::new(ExpandErrorKind::SourceMap(error)))
}

struct OperationSourceResolver<'a> {
    arguments_by_call: BTreeMap<TraceMacroCallId, &'a [ActualArgument]>,
}

impl<'a> OperationSourceResolver<'a> {
    fn new(trace: &'a Trace) -> Self {
        let arguments_by_call = trace
            .events
            .iter()
            .filter_map(|event| {
                let call = event.macro_call_id?;
                (!event.arguments.is_empty()).then_some((call, event.arguments.as_slice()))
            })
            .collect();
        Self { arguments_by_call }
    }

    fn source_for_operation(
        &self,
        call_id: TraceMacroCallId,
        argument_index: Option<u32>,
        argument_token_index: Option<u32>,
        source_map: &PreprocSourceMap,
    ) -> Option<OriginSource> {
        let argument_index = usize::try_from(argument_index?).ok()?;
        let argument_token_index = usize::try_from(argument_token_index?).ok()?;
        let argument = self.arguments_by_call.get(&call_id)?.get(argument_index)?;
        let token = argument.tokens.get(argument_token_index)?;
        source_location(source_map, token.range.as_ref()?)
    }
}

impl Origin {
    /// Translate a slang `TokenOrigin` into the hir `Origin` model.
    ///
    /// `source_map` is consulted for file-backed ranges so that the resulting
    /// `Origin` carries hir-side `TextRange`s. If a Slang buffer cannot be
    /// mapped to a file-backed range, the origin is unavailable.
    pub fn from_token_origin(
        model_file: FileId,
        origin: &TokenOrigin,
        source_map: &PreprocSourceMap,
    ) -> Option<Self> {
        Some(match origin {
            TokenOrigin::Source { token_range } => {
                let source = source_location(source_map, token_range)?;
                Origin::File { file: source.file, range: source.range }
            }
            TokenOrigin::MacroBody { call_id, definition_id, body_token_range, .. } => {
                Origin::MacroBody {
                    call: macro_call_id(model_file, *call_id),
                    def: *definition_id,
                    body_range: source_location(source_map, body_token_range)?.range,
                }
            }
            TokenOrigin::MacroArgument {
                call_id, argument_index, argument_token_range, ..
            } => Origin::MacroArg {
                call: macro_call_id(model_file, *call_id),
                arg_index: usize::try_from(*argument_index).ok()?,
                arg_range: source_location(source_map, argument_token_range)?.range,
            },
            TokenOrigin::Predefine { .. } => return None,
            TokenOrigin::TokenPaste { call_id, .. } => {
                Origin::TokenPaste { call: macro_call_id(model_file, *call_id) }
            }
            TokenOrigin::Stringify { call_id, .. } => {
                Origin::Stringify { call: macro_call_id(model_file, *call_id) }
            }
            TokenOrigin::Builtin { name, call_id, .. } if !name.is_empty() => Origin::Builtin {
                call: macro_call_id(model_file, *call_id),
                name: name.to_smolstr(),
            },
            TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => return None,
        })
    }
}

fn origin_slot_from_token_origin(
    model_file: FileId,
    emitted_token: SourceEmittedTokenId,
    origin: &TokenOrigin,
    source_map: &PreprocSourceMap,
    operation_sources: Option<&OperationSourceResolver<'_>>,
) -> Option<OriginSlot> {
    let mapped_origin = Origin::from_token_origin(model_file, origin, source_map)?;
    let source = match origin {
        TokenOrigin::Source { token_range } => source_location(source_map, token_range),
        TokenOrigin::MacroBody { body_token_range, .. } => {
            source_location(source_map, body_token_range)
        }
        TokenOrigin::MacroArgument { argument_token_range, .. } => {
            source_location(source_map, argument_token_range)
        }
        TokenOrigin::Predefine { .. } => None,
        TokenOrigin::TokenPaste { call_id, argument_index, argument_token_index, .. }
        | TokenOrigin::Stringify { call_id, argument_index, argument_token_index, .. } => {
            operation_sources.and_then(|sources| {
                sources.source_for_operation(
                    *call_id,
                    *argument_index,
                    *argument_token_index,
                    source_map,
                )
            })
        }
        TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => None,
    };
    Some(OriginSlot { emitted_token, origin: mapped_origin, source })
}

fn macro_call_id(model_file: FileId, trace_call: TraceMacroCallId) -> MacroCallId {
    MacroCallId(MacroCallLoc { model_file, trace_call })
}

fn source_location(
    source_map: &PreprocSourceMap,
    token_range: &SourceBufferRange,
) -> Option<OriginSource> {
    let source_range = source_range_from_trace(token_range)?;
    let range = match source_map.map_range(source_range) {
        Ok(range) => range,
        Err(crate::source_db::SourcePreprocQueryError::DisplayOnlyVirtualSource { .. }) => {
            return None;
        }
        Err(error) => {
            tracing::warn!(?source_range, ?error, "dropping unmapped macro expansion origin");
            return None;
        }
    };
    let file = match source_map.file_id(source_range.source) {
        Ok(file) => file,
        Err(crate::source_db::SourcePreprocQueryError::DisplayOnlyVirtualSource { .. }) => {
            return None;
        }
        Err(error) => {
            tracing::warn!(?source_range, ?error, "dropping macro expansion origin without a file");
            return None;
        }
    };
    Some(OriginSource { file, range })
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
