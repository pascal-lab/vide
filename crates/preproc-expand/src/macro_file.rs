use std::collections::BTreeMap;

use ::preproc::source::{SourceMacroCall, SourceMacroResolution, SourcePreprocModel};
use base_db::salsa;
use smol_str::SmolStr;
use syntax::{
    SyntaxTree, Trace,
    preproc::{MacroCallId as TraceMacroCallId, MacroExpansionId, TokenOrigin},
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
pub struct MacroCallId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct MacroCallLoc {
    pub model_file: FileId,
    pub trace_call: TraceMacroCallId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct MacroFileId(pub salsa::InternId);

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
    let mut model_file_ids = vec![file_id];
    for model_file_id in &db.source_preproc_contexts_for_file(file_id).model_file_ids {
        if !model_file_ids.contains(model_file_id) {
            model_file_ids.push(*model_file_id);
        }
    }

    let mut macro_files = Vec::new();
    for model_file in model_file_ids {
        let mapped = db.source_preproc_model(model_file);
        let Ok(mapped) = mapped.as_ref() else {
            continue;
        };
        let parsed = db.parsed_compilation_unit(model_file);
        let Some(trace) = parsed.preprocessor_trace.as_ref() else {
            continue;
        };
        for call in mapped.macro_call_ids_at(file_id, offset) {
            let Some(source_call) = mapped.model.macro_calls().get(call) else {
                continue;
            };
            let Some(trace_call) = source_call.trace_call else {
                continue;
            };
            if emitted_range_for_trace_call(trace, trace_call).is_none() {
                continue;
            }
            let macro_file = db.intern_macro_file(MacroCallLoc { model_file, trace_call });
            if !macro_files.contains(&macro_file) {
                macro_files.push(macro_file);
            }
        }
    }
    macro_files
}

pub fn macro_file_call_site(
    db: &dyn PreprocDb,
    macro_file: MacroFileId,
) -> Option<MacroFileCallSite> {
    let call_loc = db.lookup_intern_macro_file(macro_file);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let mapped = mapped.as_ref().as_ref().ok()?;
    let call = source_call_for_trace_call(&mapped.model, call_loc.trace_call)?;
    Some(MacroFileCallSite {
        call_file_id: mapped.source_map.file_id(call.call_range.source).ok()?,
        call_range: mapped.source_map.map_range(call.call_range).ok()?,
    })
}

pub fn macro_file_expansion(
    db: &dyn PreprocDb,
    macro_file: MacroFileId,
) -> Option<MacroFileExpansion> {
    let call_site = macro_file_call_site(db, macro_file)?;
    let call_loc = db.lookup_intern_macro_file(macro_file);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let mapped = mapped.as_ref().as_ref().ok()?;
    let call = source_call_for_trace_call(&mapped.model, call_loc.trace_call)?;
    let parsed = db.parsed_compilation_unit(call_loc.model_file);
    let trace = parsed.preprocessor_trace.as_ref()?;
    let emitted_range = emitted_range_for_trace_call(trace, call_loc.trace_call)?;
    let definition = expansion_definition(mapped, call, trace, emitted_range)?;
    Some(MacroFileExpansion {
        call_file_id: call_site.call_file_id,
        call_range: call_site.call_range,
        definition,
    })
}

pub(crate) fn macro_expansion_query(
    db: &dyn PreprocDb,
    macro_file: MacroFileId,
) -> Arc<ExpandResult<ExpansionInfo>> {
    Arc::new(macro_expansion(db, macro_file))
}

fn macro_expansion(db: &dyn PreprocDb, macro_file: MacroFileId) -> ExpandResult<ExpansionInfo> {
    let call_loc = db.lookup_intern_macro_file(macro_file);
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
    let Some(emitted_range) = emitted_range_for_trace_call(trace, call_loc.trace_call) else {
        return expansion_error(
            String::new(),
            ExpansionSourceMap::empty(),
            ExpandErrorKind::ExpansionUnavailable,
        );
    };
    let text = expansion_text_for_range(trace, emitted_range);
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
    let parse = SyntaxTree::from_text(&text, "macro-expansion", "");
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
        return map_macro_definition(mapped, definition).ok().map(MacroExpansionDefinition::Source);
    }

    // Builtin intrinsics have no source definition; identify them by the
    // origin of the tokens they emit.
    let mut builtin_names = Vec::new();
    for raw in emitted_range.start.raw()..emitted_range.start.raw() + emitted_range.len {
        let Some(token) = trace.emitted_tokens.get(raw) else {
            continue;
        };
        if let TokenOrigin::Builtin { name, .. } = &token.origin
            && !name.is_empty()
            && !builtin_names.contains(name)
        {
            builtin_names.push(name.clone());
        }
    }
    match builtin_names.as_slice() {
        [name] => Some(MacroExpansionDefinition::Builtin { name: SmolStr::new(name) }),
        _ => None,
    }
}

/// Emitted-token range of one macro expansion.
///
/// Slang emits every token of an expansion — direct body tokens, argument
/// replacements, nested expansions, and predefine tokens — as one contiguous
/// run. The range is therefore `[first, last]` over all tokens whose origin
/// expansion chain contains the call's expansion. Zero-token expansions yield
/// an empty range.
pub(crate) fn emitted_range_for_trace_call(
    trace: &Trace,
    trace_call: TraceMacroCallId,
) -> Option<SourceEmittedTokenRange> {
    let expansion_id = trace
        .events
        .iter()
        .find(|event| event.macro_call_id == Some(trace_call))?
        .macro_expansion_id?;
    let parents = expansion_parents(trace);
    let mut first = None;
    let mut last = None;
    for (index, token) in trace.emitted_tokens.iter().enumerate() {
        if token_belongs_to_expansion(&token.origin, expansion_id, &parents) {
            first.get_or_insert(index);
            last = Some(index);
        }
    }
    match last {
        Some(last) => Some(SourceEmittedTokenRange {
            start: SourceEmittedTokenId::new(first.unwrap_or_default()),
            len: last - first.unwrap_or_default() + 1,
        }),
        // Zero-token expansion: available, but empty.
        None => Some(SourceEmittedTokenRange { start: SourceEmittedTokenId::new(0), len: 0 }),
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
        | TokenOrigin::Builtin { parent_expansion_id, .. }
        | TokenOrigin::TokenPaste { parent_expansion_id, .. }
        | TokenOrigin::Stringify { parent_expansion_id, .. } => *parent_expansion_id,
        TokenOrigin::Source { .. } | TokenOrigin::Unavailable => None,
    }
}

fn token_belongs_to_expansion(
    origin: &TokenOrigin,
    target: MacroExpansionId,
    parents: &BTreeMap<MacroExpansionId, MacroExpansionId>,
) -> bool {
    let mut current = match origin {
        TokenOrigin::Source { .. } | TokenOrigin::Unavailable => return false,
        TokenOrigin::MacroBody { expansion_id, .. }
        | TokenOrigin::MacroArgument { expansion_id, .. }
        | TokenOrigin::Builtin { expansion_id, .. }
        | TokenOrigin::TokenPaste { expansion_id, .. }
        | TokenOrigin::Stringify { expansion_id, .. } => *expansion_id,
    };
    loop {
        if current == target {
            return true;
        }
        match parents.get(&current) {
            Some(parent) => current = *parent,
            None => return false,
        }
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
