use std::collections::BTreeMap;

use ::preproc::source::{SourceMacroCall, SourceMacroResolution, SourcePreprocModel};
use base_db::salsa;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use syntax::{
    SyntaxTree,
    preproc::{MacroCallId as TraceMacroCallId, MacroExpansionId, TokenOrigin, Trace},
};
use triomphe::Arc;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    db::PreprocDb,
    preproc::{MacroDefinition, map_macro_definition},
    source_db::{MappedSourcePreprocModel, SourcePreprocQueryError},
};

mod source_map;
#[cfg(test)]
mod tests;

pub use source_map::{ExpansionSourceHit, ExpansionSourceMap, ExpansionSourceMapError, Origin};

/// Index into a slang preprocessor trace's emitted token stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceEmittedTokenId(usize);

impl SourceEmittedTokenId {
    pub fn new(raw: usize) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> usize {
        self.0
    }
}

/// A contiguous range of emitted tokens in trace order. Slang emits the tokens
/// of one expansion (including nested expansions and predefine tokens)
/// contiguously, so an expansion is fully described by its first token and
/// length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceEmittedTokenRange {
    pub start: SourceEmittedTokenId,
    pub len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct MacroCallLoc {
    pub model_file: FileId,
    pub trace_call: TraceMacroCallId,
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct MacroCallId {
    #[returns(copy)]
    pub loc: MacroCallLoc,
}

impl PartialOrd for MacroCallId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MacroCallId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        salsa::plumbing::AsId::as_id(self).cmp(&salsa::plumbing::AsId::as_id(other))
    }
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct MacroFileId {
    #[returns(copy)]
    pub loc: MacroCallLoc,
}

impl PartialOrd for MacroFileId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MacroFileId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        salsa::plumbing::AsId::as_id(self).cmp(&salsa::plumbing::AsId::as_id(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionInfo {
    pub text: String,
    pub parse: SyntaxTree,
    pub source_map: ExpansionSourceMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandResult<T> {
    pub value: T,
    pub err: Option<ExpandError>,
}

impl<T> ExpandResult<T> {
    pub fn ok(value: T) -> Self {
        Self { value, err: None }
    }

    pub fn new(value: T, err: ExpandError) -> Self {
        Self { value, err: Some(err) }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ExpandResult<U> {
        ExpandResult { value: f(self.value), err: self.err }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandError {
    kind: ExpandErrorKind,
}

impl ExpandError {
    pub fn new(kind: ExpandErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> &ExpandErrorKind {
        &self.kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandErrorKind {
    SourcePreprocModel(SourcePreprocQueryError),
    MissingTraceCall { trace_call: TraceMacroCallId },
    ExpansionUnavailable,
    InvalidEmittedTokenRange { start: SourceEmittedTokenId, len: usize },
    MissingEmittedToken { token: SourceEmittedTokenId },
    TraceUnavailable,
    SourceMap(ExpansionSourceMapError),
}

/// Information about one macro expansion at the call site, exposed to the IDE.
///
/// `call_file_id` and `call_range` are already mapped to the user-facing file
/// and range. `definition` is `Builtin` for intrinsics and otherwise the
/// resolved [`MacroDefinition`] reused from the preproc query layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroFileCallSite {
    pub call_file_id: FileId,
    pub call_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroFileExpansion {
    pub call_file_id: FileId,
    pub call_range: TextRange,
    pub definition: MacroExpansionDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroExpansionDefinition {
    Source(MacroDefinition),
    Builtin { name: SmolStr },
}

pub fn macro_files_at_offset(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> Vec<MacroFileId> {
    let Some(model_file_ids) = relevant_model_files(db, file_id) else {
        return Vec::new();
    };

    let mut macro_files = Vec::new();
    for model_file in model_file_ids {
        let mapped = db.source_preproc_model(model_file);
        let mapped = match mapped.as_ref() {
            Ok(mapped) => mapped,
            Err(error) => {
                tracing::warn!(
                    ?file_id,
                    ?model_file,
                    ?error,
                    "macro expansion candidates unavailable for preprocessor model"
                );
                return Vec::new();
            }
        };
        let parsed = db.parsed_compilation_unit(model_file);
        if parsed.preprocessor_trace.is_none() {
            tracing::warn!(
                ?file_id,
                ?model_file,
                "macro expansion candidates unavailable without a preprocessor trace"
            );
            return Vec::new();
        }
        for call in mapped.macro_call_ids_at(file_id, offset) {
            let Some(source_call) = mapped.model.macro_calls().get(call) else {
                tracing::warn!(
                    ?file_id,
                    ?model_file,
                    ?call,
                    "macro expansion index references a missing call"
                );
                return Vec::new();
            };
            let Some(trace_call) = source_call.trace_call else {
                tracing::warn!(
                    ?file_id,
                    ?model_file,
                    ?call,
                    "macro call has no Slang trace identity"
                );
                return Vec::new();
            };
            if db.trace_index(model_file).emitted_range_for_call(trace_call).is_none() {
                tracing::warn!(
                    ?file_id,
                    ?model_file,
                    ?trace_call,
                    "macro call has no Slang emitted-token range"
                );
                return Vec::new();
            }
            let macro_file = MacroFileId::new(db, MacroCallLoc { model_file, trace_call });
            if !macro_files.contains(&macro_file) {
                macro_files.push(macro_file);
            }
        }
    }
    macro_files
}

/// Returns every macro expansion whose invocation is mapped to `file_id`.
///
/// Workspace indexes need definitions emitted by macros even when the query
/// position is an ordinary HDL reference elsewhere in the file. Building
/// this set from the preprocessor model keeps macro-generated design units in
/// the same module index as source-written units without inventing a second
/// name-resolution path.
pub fn macro_files_for_file(db: &dyn PreprocDb, file_id: FileId) -> Vec<MacroFileId> {
    let Some(model_file_ids) = relevant_model_files(db, file_id) else {
        return Vec::new();
    };
    let mut macro_files = Vec::new();
    for model_file in model_file_ids {
        let mapped = db.source_preproc_model(model_file);
        let mapped = match mapped.as_ref() {
            Ok(mapped) => mapped,
            Err(error) => {
                tracing::warn!(
                    ?file_id,
                    ?model_file,
                    ?error,
                    "macro file index unavailable for preprocessor model"
                );
                return Vec::new();
            }
        };
        let parsed = db.parsed_compilation_unit(model_file);
        if parsed.preprocessor_trace.is_none() {
            tracing::warn!(
                ?file_id,
                ?model_file,
                "macro file index unavailable without a preprocessor trace"
            );
            return Vec::new();
        }
        for call in mapped.model.macro_calls().iter() {
            let call_file = match mapped.source_map.file_id(call.call_range.source) {
                Ok(call_file) => call_file,
                Err(error) => {
                    tracing::warn!(
                        ?file_id,
                        ?model_file,
                        ?error,
                        "macro call source mapping unavailable"
                    );
                    return Vec::new();
                }
            };
            if call_file != file_id {
                continue;
            }
            let Some(trace_call) = call.trace_call else {
                tracing::warn!(
                    ?file_id,
                    ?model_file,
                    call_id = call.id.raw(),
                    "macro call has no Slang trace identity"
                );
                return Vec::new();
            };
            if db.trace_index(model_file).emitted_range_for_call(trace_call).is_none() {
                tracing::warn!(
                    ?file_id,
                    ?model_file,
                    ?trace_call,
                    "macro call has no Slang emitted-token range"
                );
                return Vec::new();
            }
            let macro_file = MacroFileId::new(db, MacroCallLoc { model_file, trace_call });
            if !macro_files.contains(&macro_file) {
                macro_files.push(macro_file);
            }
        }
    }
    macro_files
}

fn relevant_model_files(db: &dyn PreprocDb, file_id: FileId) -> Option<Vec<FileId>> {
    let contexts = db.source_preproc_contexts_for_file(file_id);
    if let crate::source_db::SourcePreprocContextStatus::Partial { skipped_models } =
        contexts.status
    {
        tracing::warn!(
            ?file_id,
            skipped_models,
            "macro expansion query unavailable because preprocessor contexts are partial"
        );
        return None;
    }

    let mut model_file_ids = vec![file_id];
    for model_file_id in &contexts.model_file_ids {
        if !model_file_ids.contains(model_file_id) {
            model_file_ids.push(*model_file_id);
        }
    }
    Some(model_file_ids)
}

pub fn macro_file_call_site(
    db: &dyn PreprocDb,
    macro_file: MacroFileId,
) -> Option<MacroFileCallSite> {
    let call_loc = macro_file.loc(db);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let mapped = match mapped.as_ref().as_ref() {
        Ok(mapped) => mapped,
        Err(error) => {
            tracing::warn!(
                ?macro_file,
                ?error,
                "macro call site unavailable for preprocessor model"
            );
            return None;
        }
    };
    let Some(call) = source_call_for_trace_call(&mapped.model, call_loc.trace_call) else {
        tracing::warn!(
            ?macro_file,
            "macro call site has no source call for its Slang trace identity"
        );
        return None;
    };
    let call_file_id = match mapped.source_map.file_id(call.call_range.source) {
        Ok(file_id) => file_id,
        Err(error) => {
            tracing::warn!(?macro_file, ?error, "macro call site source file mapping failed");
            return None;
        }
    };
    let call_range = match mapped.source_map.map_range(call.call_range) {
        Ok(range) => range,
        Err(error) => {
            tracing::warn!(?macro_file, ?error, "macro call site range mapping failed");
            return None;
        }
    };
    Some(MacroFileCallSite { call_file_id, call_range })
}

pub fn macro_file_expansion(
    db: &dyn PreprocDb,
    macro_file: MacroFileId,
) -> Option<MacroFileExpansion> {
    let call_site = macro_file_call_site(db, macro_file)?;
    let call_loc = macro_file.loc(db);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let mapped = match mapped.as_ref().as_ref() {
        Ok(mapped) => mapped,
        Err(error) => {
            tracing::warn!(
                ?macro_file,
                ?error,
                "macro expansion metadata unavailable for preprocessor model"
            );
            return None;
        }
    };
    let Some(call) = source_call_for_trace_call(&mapped.model, call_loc.trace_call) else {
        tracing::warn!(?macro_file, "macro expansion has no source call for its trace identity");
        return None;
    };
    let parsed = db.parsed_compilation_unit(call_loc.model_file);
    let Some(trace) = parsed.preprocessor_trace.as_ref() else {
        tracing::warn!(?macro_file, "macro expansion has no preprocessor trace");
        return None;
    };
    let Some(emitted_range) =
        db.trace_index(call_loc.model_file).emitted_range_for_call(call_loc.trace_call)
    else {
        tracing::warn!(?macro_file, "macro expansion has no emitted-token range");
        return None;
    };
    let Some(definition) = expansion_definition(mapped, call, trace, emitted_range) else {
        tracing::warn!(?macro_file, "macro expansion has no source or builtin definition");
        return None;
    };
    Some(MacroFileExpansion {
        call_file_id: call_site.call_file_id,
        call_range: call_site.call_range,
        definition,
    })
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn macro_expansion_query(
    db: &dyn PreprocDb,
    macro_file: MacroFileId,
) -> Arc<ExpandResult<ExpansionInfo>> {
    Arc::new(macro_expansion(db, macro_file))
}

fn macro_expansion(db: &dyn PreprocDb, macro_file: MacroFileId) -> ExpandResult<ExpansionInfo> {
    let call_loc = macro_file.loc(db);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let mapped = match mapped.as_ref() {
        Ok(mapped) => mapped,
        Err(err) => {
            return expansion_error(
                String::new(),
                ExpansionSourceMap::empty(),
                ExpandErrorKind::SourcePreprocModel(err.clone()),
            );
        }
    };
    let Some(_call) = source_call_for_trace_call(&mapped.model, call_loc.trace_call) else {
        return expansion_error(
            String::new(),
            ExpansionSourceMap::empty(),
            ExpandErrorKind::MissingTraceCall { trace_call: call_loc.trace_call },
        );
    };
    let parsed = db.parsed_compilation_unit(call_loc.model_file);
    let Some(trace) = parsed.preprocessor_trace.as_ref() else {
        return expansion_error(
            String::new(),
            ExpansionSourceMap::empty(),
            ExpandErrorKind::TraceUnavailable,
        );
    };
    let Some(emitted_range) =
        db.trace_index(call_loc.model_file).emitted_range_for_call(call_loc.trace_call)
    else {
        return expansion_error(
            String::new(),
            ExpansionSourceMap::empty(),
            ExpandErrorKind::ExpansionUnavailable,
        );
    };
    let text = if emitted_range.len == 0 {
        expansion_text_for_empty_call(trace, call_loc.trace_call)
    } else {
        expansion_text_for_range(trace, emitted_range)
    };
    let source_map = ExpansionSourceMap::from_trace_range(
        db,
        call_loc.model_file,
        trace,
        &mapped.source_map,
        emitted_range,
    );
    expansion_info_from_parts(text, source_map)
}

fn expansion_info_from_parts(
    text: ExpandResult<String>,
    source_map: ExpandResult<ExpansionSourceMap>,
) -> ExpandResult<ExpansionInfo> {
    let err = text.err.or(source_map.err);
    expansion_info(text.value, source_map.value, err)
}

fn expansion_error(
    text: String,
    source_map: ExpansionSourceMap,
    kind: ExpandErrorKind,
) -> ExpandResult<ExpansionInfo> {
    expansion_info(text, source_map, Some(ExpandError::new(kind)))
}

fn expansion_info(
    text: String,
    source_map: ExpansionSourceMap,
    err: Option<ExpandError>,
) -> ExpandResult<ExpansionInfo> {
    let parse = SyntaxTree::from_file_in_memory(&text, "macro-expansion", "");
    ExpandResult { value: ExpansionInfo { text, parse, source_map }, err }
}

fn expansion_definition(
    mapped: &MappedSourcePreprocModel,
    call: &SourceMacroCall,
    trace: &Trace,
    emitted_range: SourceEmittedTokenRange,
) -> Option<MacroExpansionDefinition> {
    let reference = mapped.model.macro_references().get(call.reference)?;
    if let SourceMacroResolution::Resolved { definition, .. } = &reference.resolution {
        let definition = mapped.model.macro_definitions().get(*definition)?;
        return match map_macro_definition(mapped, definition) {
            Ok(definition) => Some(MacroExpansionDefinition::Source(definition)),
            Err(error) => {
                tracing::warn!(
                    event_id = definition.event_id.raw(),
                    ?error,
                    "macro expansion source definition mapping failed"
                );
                None
            }
        };
    }

    // Builtin intrinsics have no source definition; identify them by the
    // origin of the tokens they emit.
    let mut builtin_names = Vec::<String>::new();
    for raw in emitted_range.start.raw()..emitted_range.start.raw() + emitted_range.len {
        let Some(token) = trace.emitted_tokens.get(raw) else {
            continue;
        };
        if let TokenOrigin::Builtin { name, .. } = &token.origin
            && !name.is_empty()
            && !builtin_names.contains(name)
        {
            builtin_names.push(name.to_owned());
        }
    }
    match builtin_names.as_slice() {
        [name] => Some(MacroExpansionDefinition::Builtin { name: SmolStr::new(name) }),
        _ => None,
    }
}

/// Precomputed lookup tables over one slang preprocessor trace.
///
/// Both tables are built in a single pass over the trace (usage events for
/// the call→expansion map, emitted tokens for the expansion→range map) so
/// that per-call queries are O(1) instead of re-scanning events and emitted
/// tokens on every request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraceIndex {
    expansion_by_call: FxHashMap<TraceMacroCallId, MacroExpansionId>,
    emitted_range_by_expansion: FxHashMap<MacroExpansionId, (usize, usize)>,
}

impl TraceIndex {
    pub(crate) fn new(trace: &Trace) -> Self {
        let mut expansion_by_call = FxHashMap::default();
        for event in &trace.events {
            if let (Some(call), Some(expansion)) = (event.macro_call_id, event.macro_expansion_id) {
                expansion_by_call.entry(call).or_insert(expansion);
            }
        }

        let parents = expansion_parents(trace);
        let mut first_by_expansion: FxHashMap<MacroExpansionId, usize> = FxHashMap::default();
        let mut last_by_expansion: FxHashMap<MacroExpansionId, usize> = FxHashMap::default();
        for (index, token) in trace.emitted_tokens.iter().enumerate() {
            let Some(expansion) = token_origin_expansion(&token.origin) else {
                continue;
            };
            // A token belongs to its own expansion and every ancestor
            // expansion, mirroring `token_belongs_to_expansion`.
            let mut current = expansion;
            loop {
                first_by_expansion.entry(current).or_insert(index);
                last_by_expansion.insert(current, index);
                match parents.get(&current) {
                    Some(parent) => current = *parent,
                    None => break,
                }
            }
        }

        let emitted_range_by_expansion = first_by_expansion
            .into_iter()
            .map(|(expansion, first)| (expansion, (first, last_by_expansion[&expansion])))
            .collect();
        Self { expansion_by_call, emitted_range_by_expansion }
    }

    /// Emitted-token range of one macro expansion, or `None` when the call has
    /// no recorded expansion. Zero-token expansions yield an empty range.
    pub(crate) fn emitted_range_for_call(
        &self,
        trace_call: TraceMacroCallId,
    ) -> Option<SourceEmittedTokenRange> {
        let expansion_id = *self.expansion_by_call.get(&trace_call)?;
        match self.emitted_range_by_expansion.get(&expansion_id) {
            Some((first, last)) => Some(SourceEmittedTokenRange {
                start: SourceEmittedTokenId::new(*first),
                len: last - first + 1,
            }),
            None => Some(SourceEmittedTokenRange { start: SourceEmittedTokenId::new(0), len: 0 }),
        }
    }
}

#[salsa::tracked(returns(clone), lru = 128)]
pub(crate) fn trace_index_query(
    db: &dyn PreprocDb,
    key: crate::db::PreprocFileQueryKey,
) -> Arc<TraceIndex> {
    let model_file = key.file_id(db);
    let parsed = db.parsed_compilation_unit(model_file);
    match parsed.preprocessor_trace.as_ref() {
        Some(trace) => Arc::new(TraceIndex::new(trace)),
        None => Arc::new(TraceIndex::default()),
    }
}

/// Parent-expansion links. Slang records them on each emitted token's origin
/// (the expansion chain of the token), not on the usage events, so the map is
/// built from token origins.
fn expansion_parents(trace: &Trace) -> BTreeMap<MacroExpansionId, MacroExpansionId> {
    let mut parents = BTreeMap::new();
    for token in &trace.emitted_tokens {
        if let (Some(expansion), Some(parent)) =
            (token_origin_expansion(&token.origin), token_origin_parent_expansion(&token.origin))
        {
            parents.entry(expansion).or_insert(parent);
        }
    }
    parents
}

fn token_origin_expansion(origin: &TokenOrigin) -> Option<MacroExpansionId> {
    match origin {
        TokenOrigin::MacroBody { expansion_id, .. }
        | TokenOrigin::MacroArgument { expansion_id, .. }
        | TokenOrigin::Predefine { expansion_id, .. }
        | TokenOrigin::Builtin { expansion_id, .. }
        | TokenOrigin::TokenPaste { expansion_id, .. }
        | TokenOrigin::Stringify { expansion_id, .. } => Some(*expansion_id),
        TokenOrigin::Source { .. } | TokenOrigin::Unavailable => None,
    }
}

fn token_origin_parent_expansion(origin: &TokenOrigin) -> Option<MacroExpansionId> {
    match origin {
        TokenOrigin::MacroBody { parent_expansion_id, .. }
        | TokenOrigin::MacroArgument { parent_expansion_id, .. }
        | TokenOrigin::Predefine { parent_expansion_id, .. }
        | TokenOrigin::Builtin { parent_expansion_id, .. }
        | TokenOrigin::TokenPaste { parent_expansion_id, .. }
        | TokenOrigin::Stringify { parent_expansion_id, .. } => *parent_expansion_id,
        TokenOrigin::Source { .. } | TokenOrigin::Unavailable => None,
    }
}

fn source_call_for_trace_call(
    model: &SourcePreprocModel,
    trace_call: TraceMacroCallId,
) -> Option<&SourceMacroCall> {
    model.macro_calls().iter().find(|call| call.trace_call == Some(trace_call))
}

fn expansion_text_for_range(
    trace: &Trace,
    emitted_range: SourceEmittedTokenRange,
) -> ExpandResult<String> {
    let mut text = String::new();
    let start = emitted_range.start.raw();
    if start > trace.emitted_tokens.len() {
        return ExpandResult::new(
            text,
            ExpandError::new(ExpandErrorKind::InvalidEmittedTokenRange {
                start: emitted_range.start,
                len: emitted_range.len,
            }),
        );
    }
    let Some(end) = start.checked_add(emitted_range.len) else {
        return ExpandResult::new(
            text,
            ExpandError::new(ExpandErrorKind::InvalidEmittedTokenRange {
                start: emitted_range.start,
                len: emitted_range.len,
            }),
        );
    };
    for raw in start..end {
        let token = SourceEmittedTokenId::new(raw);
        let Some(token_data) = trace.emitted_tokens.get(raw) else {
            return ExpandResult::new(
                text,
                ExpandError::new(ExpandErrorKind::MissingEmittedToken { token }),
            );
        };
        text.push_str(token_data.display_text.as_str());
    }
    ExpandResult::ok(text)
}

fn expansion_text_for_empty_call(
    trace: &Trace,
    trace_call: TraceMacroCallId,
) -> ExpandResult<String> {
    let Some(event) = trace.events.iter().find(|event| event.macro_call_id == Some(trace_call))
    else {
        return ExpandResult::new(
            String::new(),
            ExpandError::new(ExpandErrorKind::MissingTraceCall { trace_call }),
        );
    };

    let body_tokens = if event.body_tokens.is_empty() {
        event
            .macro_definition_id
            .and_then(|definition_id| {
                trace.events.iter().find(|candidate| {
                    candidate.macro_call_id.is_none()
                        && candidate.macro_definition_id == Some(definition_id)
                        && !candidate.body_tokens.is_empty()
                })
            })
            .map_or(event.body_tokens.as_slice(), |definition| definition.body_tokens.as_slice())
    } else {
        event.body_tokens.as_slice()
    };
    let Some(first) = body_tokens.first() else {
        return ExpandResult::ok(String::new());
    };
    let Some(last) = body_tokens.last() else {
        unreachable!("event body token list became empty after first-token check");
    };
    if let (Some(first_range), Some(last_range)) = (&first.range, &last.range)
        && first_range.buffer_id == last_range.buffer_id
        && let Some(buffer) =
            trace.source_buffers.iter().find(|buffer| buffer.buffer_id == first_range.buffer_id)
        && let Some(text) = buffer.text.as_deref()
        && let Some(body) = text.get(first_range.range.start..last_range.range.end)
    {
        return ExpandResult::ok(body.to_owned());
    }

    ExpandResult::ok(body_tokens.iter().map(|token| token.raw_text.as_str()).collect())
}
