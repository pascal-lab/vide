use ::preproc::source::{
    SourceEmittedTokenRange, SourceMacroCall, SourceMacroCallId, SourceMacroExpansion,
    SourceMacroExpansionDefinition, SourcePreprocModel, SourcePreprocUnavailable,
};
use smol_str::SmolStr;
use syntax::{SyntaxTree, preproc::MacroCallId as TraceMacroCallId};
use triomphe::Arc;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    base_db::{
        salsa,
        source_db::{MappedSourcePreprocModel, SourcePreprocQueryError},
    },
    db::HirDb,
    preproc::{MacroDefinition, map_macro_definition},
};

mod source_map;
#[cfg(test)]
mod tests;

pub use ::preproc::source::SourceEmittedTokenId;
pub use source_map::{ExpansionSourceHit, ExpansionSourceMap, ExpansionSourceMapError, Origin};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct MacroCallId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct MacroCallLoc {
    pub model_file: FileId,
    pub trace_call: TraceMacroCallId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct MacroFileId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct MacroFileLoc {
    pub call: MacroCallId,
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
    ExpansionUnavailable(SourcePreprocUnavailable),
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
    db: &dyn HirDb,
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
        for call in mapped.macro_call_ids_at(file_id, offset) {
            if emitted_range_for_call(&mapped.model, call).is_none() {
                continue;
            }
            let Some(source_call) = mapped.model.macro_calls().get(call) else {
                continue;
            };
            let Some(macro_call) = macro_call_for_source_call(db, model_file, source_call) else {
                continue;
            };
            let macro_file = db.intern_macro_file(MacroFileLoc { call: macro_call });
            if !macro_files.contains(&macro_file) {
                macro_files.push(macro_file);
            }
        }
    }
    macro_files
}

pub fn macro_file_call_site(db: &dyn HirDb, macro_file: MacroFileId) -> Option<MacroFileCallSite> {
    let loc = db.lookup_intern_macro_file(macro_file);
    let call_loc = db.lookup_intern_macro_call(loc.call);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let mapped = mapped.as_ref().as_ref().ok()?;
    let call = source_call_for_trace_call(&mapped.model, call_loc.trace_call)?;
    Some(MacroFileCallSite {
        call_file_id: mapped.source_map.file_id(call.call_range.source).ok()?,
        call_range: mapped.source_map.map_range(call.call_range).ok()?,
    })
}

pub fn macro_file_expansion(db: &dyn HirDb, macro_file: MacroFileId) -> Option<MacroFileExpansion> {
    let call_site = macro_file_call_site(db, macro_file)?;
    let loc = db.lookup_intern_macro_file(macro_file);
    let call_loc = db.lookup_intern_macro_call(loc.call);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let mapped = mapped.as_ref().as_ref().ok()?;
    let call = source_call_for_trace_call(&mapped.model, call_loc.trace_call)?;
    let expansion = source_expansion_for_call(&mapped.model, call.id).ok()?;
    Some(MacroFileExpansion {
        call_file_id: call_site.call_file_id,
        call_range: call_site.call_range,
        definition: macro_expansion_definition(mapped, expansion)?,
    })
}

pub(crate) fn macro_expansion_query(
    db: &dyn HirDb,
    macro_file: MacroFileId,
) -> Arc<ExpandResult<ExpansionInfo>> {
    Arc::new(macro_expansion(db, macro_file))
}

fn macro_expansion(db: &dyn HirDb, macro_file: MacroFileId) -> ExpandResult<ExpansionInfo> {
    let loc = db.lookup_intern_macro_file(macro_file);
    let call_loc = db.lookup_intern_macro_call(loc.call);
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
    let Some(call) = source_call_for_trace_call(&mapped.model, call_loc.trace_call) else {
        return expansion_error(
            String::new(),
            ExpansionSourceMap::empty(),
            ExpandErrorKind::MissingTraceCall { trace_call: call_loc.trace_call },
        );
    };
    let expansion = match source_expansion_for_call(&mapped.model, call.id) {
        Ok(expansion) => expansion,
        Err(err) => {
            return expansion_error(
                String::new(),
                ExpansionSourceMap::empty(),
                ExpandErrorKind::ExpansionUnavailable(err),
            );
        }
    };
    let emitted_range = expansion.emitted_token_range;
    let text = expansion_text_for_range(&mapped.model, emitted_range);
    let parsed = db.parsed_compilation_unit(call_loc.model_file);
    let source_map = match parsed.preprocessor_trace.as_ref() {
        Some(trace) => ExpansionSourceMap::from_trace_range(
            db,
            call_loc.model_file,
            trace,
            &mapped.source_map,
            emitted_range,
        ),
        None => ExpandResult::new(
            ExpansionSourceMap::empty(),
            ExpandError::new(ExpandErrorKind::TraceUnavailable),
        ),
    };
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

fn macro_call_for_source_call(
    db: &dyn HirDb,
    model_file: FileId,
    call: &SourceMacroCall,
) -> Option<MacroCallId> {
    let trace_call = call.trace_call?;
    Some(db.intern_macro_call(MacroCallLoc { model_file, trace_call }))
}

fn macro_expansion_definition(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansion,
) -> Option<MacroExpansionDefinition> {
    match &expansion.definition {
        SourceMacroExpansionDefinition::Source(definition) => {
            let definition = mapped.model.macro_definitions().get(*definition)?;
            map_macro_definition(mapped, definition).ok().map(MacroExpansionDefinition::Source)
        }
        SourceMacroExpansionDefinition::Builtin { name } => {
            Some(MacroExpansionDefinition::Builtin { name: name.clone() })
        }
    }
}

fn emitted_range_for_call(
    model: &SourcePreprocModel,
    call: SourceMacroCallId,
) -> Option<SourceEmittedTokenRange> {
    source_expansion_for_call(model, call).ok().map(|expansion| expansion.emitted_token_range)
}

fn source_call_for_trace_call(
    model: &SourcePreprocModel,
    trace_call: TraceMacroCallId,
) -> Option<&SourceMacroCall> {
    model.macro_calls().iter().find(|call| call.trace_call == Some(trace_call))
}

fn source_expansion_for_call(
    model: &SourcePreprocModel,
    call: SourceMacroCallId,
) -> Result<&SourceMacroExpansion, SourcePreprocUnavailable> {
    let expansion = model.immediate_macro_expansion(call)?;
    model
        .macro_expansions()
        .get(expansion)
        .ok_or(SourcePreprocUnavailable::MissingMacroExpansion { call })
}

fn expansion_text_for_range(
    model: &SourcePreprocModel,
    emitted_range: SourceEmittedTokenRange,
) -> ExpandResult<String> {
    let mut text = String::new();
    let start = emitted_range.start.raw();
    if start > model.emitted_tokens().len() {
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
        let Some(token_data) = model.emitted_tokens().get(token) else {
            return ExpandResult::new(
                text,
                ExpandError::new(ExpandErrorKind::MissingEmittedToken { token }),
            );
        };
        text.push_str(token_data.display.as_str());
    }
    ExpandResult::ok(text)
}
