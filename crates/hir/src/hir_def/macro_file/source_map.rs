use std::collections::BTreeMap;

use ::preproc::source::{PreprocSourceId, SourceEmittedTokenRange, SourceRange};
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SourceBufferRange,
    preproc::{
        ActualArgument, MacroCallId as TraceMacroCallId, MacroDefinitionId, MacroOperationOrigin,
        TokenOrigin, Trace,
    },
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use super::{MacroCallId, MacroCallLoc};
use crate::{base_db::source_db::PreprocSourceMap, db::HirDb};

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OriginSlot {
    origin: Origin,
    source: Option<OriginSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OriginSource {
    file: FileId,
    range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionSourceHit {
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

    pub fn map_up(&self, expanded_token_index: usize) -> Option<Origin> {
        self.origins
            .get(expanded_token_index)
            .and_then(|slot| slot.as_ref().map(|slot| slot.origin.clone()))
    }

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
        self.origins
            .iter()
            .enumerate()
            .filter_map(|(expanded_token_index, slot)| {
                let slot = slot.as_ref()?;
                let source = slot.source?;
                (source.file == file && source.range.contains(offset)).then(|| ExpansionSourceHit {
                    expanded_token_index,
                    range: source.range,
                    origin: slot.origin.clone(),
                })
            })
            .collect()
    }

    pub(crate) fn from_trace_range(
        db: &dyn HirDb,
        model_file: FileId,
        trace: &Trace,
        source_map: &PreprocSourceMap,
        emitted_range: SourceEmittedTokenRange,
    ) -> Self {
        let Some(end) = emitted_range.start.raw().checked_add(emitted_range.len) else {
            return Self::empty();
        };
        let operation_sources = OperationSourceResolver::new(trace);
        let origins = (emitted_range.start.raw()..end)
            .map(|raw| {
                trace.emitted_tokens.get(raw).and_then(|token| {
                    origin_slot_from_token_origin(
                        db,
                        model_file,
                        &token.origin,
                        source_map,
                        Some(&operation_sources),
                    )
                })
            })
            .collect();
        Self { origins }
    }

    #[cfg(test)]
    pub(crate) fn from_token_origins<'a>(
        db: &dyn HirDb,
        model_file: FileId,
        origins: impl IntoIterator<Item = &'a TokenOrigin>,
        source_map: &PreprocSourceMap,
    ) -> Self {
        Self {
            origins: origins
                .into_iter()
                .map(|origin| {
                    origin_slot_from_token_origin(db, model_file, origin, source_map, None)
                })
                .collect(),
        }
    }
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
        origin: &MacroOperationOrigin,
        source_map: &PreprocSourceMap,
    ) -> Option<OriginSource> {
        let argument_index = usize::try_from(origin.argument_index?).ok()?;
        let argument_token_index = usize::try_from(origin.argument_token_index?).ok()?;
        let argument = self.arguments_by_call.get(&origin.call_id)?.get(argument_index)?;
        let token = argument.tokens.get(argument_token_index)?;
        source_location(source_map, token.range.as_ref()?)
    }
}

fn origin_slot_from_token_origin(
    db: &dyn HirDb,
    model_file: FileId,
    origin: &TokenOrigin,
    source_map: &PreprocSourceMap,
    operation_sources: Option<&OperationSourceResolver<'_>>,
) -> Option<OriginSlot> {
    match origin {
        TokenOrigin::Source { token_range } => {
            let source = source_location(source_map, token_range)?;
            Some(OriginSlot {
                origin: Origin::File { file: source.file, range: source.range },
                source: Some(source),
            })
        }
        TokenOrigin::MacroBody { origin, body_token_range, .. } => Some(Origin::MacroBody {
            call: macro_call_id(db, model_file, origin.call_id),
            def: origin.definition_id,
            body_range: source_location(source_map, body_token_range)
                .map_or(text_range(body_token_range)?, |source| source.range),
        })
        .map(|origin| OriginSlot { origin, source: source_location(source_map, body_token_range) }),
        TokenOrigin::MacroArgument { origin, argument_token_range, .. } => Some(OriginSlot {
            origin: Origin::MacroArg {
                call: macro_call_id(db, model_file, origin.call_id),
                arg_index: usize::try_from(origin.argument_index).ok()?,
                arg_range: source_location(source_map, argument_token_range)
                    .map_or(text_range(argument_token_range)?, |source| source.range),
            },
            source: source_location(source_map, argument_token_range),
        }),
        TokenOrigin::TokenPaste { origin } => Some(OriginSlot {
            origin: Origin::TokenPaste { call: macro_call_id(db, model_file, origin.call_id) },
            source: operation_sources
                .and_then(|sources| sources.source_for_operation(origin, source_map)),
        }),
        TokenOrigin::Stringify { origin } => Some(OriginSlot {
            origin: Origin::Stringify { call: macro_call_id(db, model_file, origin.call_id) },
            source: operation_sources
                .and_then(|sources| sources.source_for_operation(origin, source_map)),
        }),
        TokenOrigin::Builtin { name, origin } if !name.is_empty() => Some(OriginSlot {
            origin: Origin::Builtin {
                call: macro_call_id(db, model_file, origin.call_id),
                name: name.to_smolstr(),
            },
            source: None,
        }),
        TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => None,
    }
}

fn macro_call_id(db: &dyn HirDb, model_file: FileId, trace_call: TraceMacroCallId) -> MacroCallId {
    db.intern_macro_call(MacroCallLoc { model_file, trace_call })
}

fn source_location(
    source_map: &PreprocSourceMap,
    token_range: &SourceBufferRange,
) -> Option<OriginSource> {
    let source_range = source_range_from_trace(token_range)?;
    let range = source_map.map_range(source_range).ok()?;
    let file = source_map.file_id(source_range.source).ok()?;
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
