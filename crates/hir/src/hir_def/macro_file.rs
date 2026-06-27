use ::preproc::source::{
    SourceEmittedTokenRange, SourceMacroCall, SourceMacroCallId, SourceMacroExpansion,
    SourceMacroExpansionDefinition, SourcePreprocModel,
};
use smol_str::SmolStr;
use syntax::{SyntaxTree, preproc::MacroCallId as TraceMacroCallId};
use triomphe::Arc;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    base_db::{salsa, source_db::MappedSourcePreprocModel},
    db::HirDb,
    preproc::{MacroDefinition, map_macro_definition},
};

mod source_map;
#[cfg(test)]
mod tests;

pub use ::preproc::source::SourceEmittedTokenId;
pub use source_map::{ExpansionSourceHit, ExpansionSourceMap, Origin};

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

/// Information about one macro expansion at the call site, exposed to the IDE.
///
/// `call_file_id` and `call_range` are already mapped to the user-facing file
/// and range. `definition` is `Builtin` for intrinsics and otherwise the
/// resolved [`MacroDefinition`] reused from the preproc query layer.
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

pub fn macro_file_expansion(db: &dyn HirDb, macro_file: MacroFileId) -> Option<MacroFileExpansion> {
    let loc = db.lookup_intern_macro_file(macro_file);
    let call_loc = db.lookup_intern_macro_call(loc.call);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let mapped = mapped.as_ref().as_ref().ok()?;
    let call = source_call_for_trace_call(&mapped.model, call_loc.trace_call)?;
    let expansion = source_expansion_for_call(&mapped.model, call.id)?;
    let (call_file_id, call_range) = mapped.source_map.map_range(call.call_range).ok()?;
    Some(MacroFileExpansion {
        call_file_id,
        call_range,
        definition: macro_expansion_definition(mapped, expansion)?,
    })
}

pub(crate) fn macro_expansion_query(db: &dyn HirDb, macro_file: MacroFileId) -> Arc<ExpansionInfo> {
    let loc = db.lookup_intern_macro_file(macro_file);
    let call_loc = db.lookup_intern_macro_call(loc.call);
    let mapped = db.source_preproc_model(call_loc.model_file);
    let expansion = mapped.as_ref().as_ref().ok().and_then(|mapped| {
        source_call_for_trace_call(&mapped.model, call_loc.trace_call)
            .and_then(|call| emitted_range_for_call(&mapped.model, call.id))
            .map(|range| (mapped, range))
    });
    let (text, source_map) = expansion
        .map(|(mapped, emitted_range)| {
            let text = expansion_text_for_range(&mapped.model, emitted_range).unwrap_or_default();
            let source_map = db
                .parsed_compilation_unit(call_loc.model_file)
                .preprocessor_trace
                .as_ref()
                .map(|trace| {
                    ExpansionSourceMap::from_trace_range(
                        db,
                        call_loc.model_file,
                        trace,
                        &mapped.source_map,
                        emitted_range,
                    )
                })
                .unwrap_or_else(ExpansionSourceMap::empty);
            (text, source_map)
        })
        .unwrap_or_else(|| (String::new(), ExpansionSourceMap::empty()));
    let parse = SyntaxTree::from_text(&text, "macro-expansion", "");
    Arc::new(ExpansionInfo { text, parse, source_map })
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
    source_expansion_for_call(model, call).map(|expansion| expansion.emitted_token_range)
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
) -> Option<&SourceMacroExpansion> {
    let expansion = match model.immediate_macro_expansion(call) {
        Ok(expansion) => model.macro_expansions().get(expansion)?,
        Err(_) => return None,
    };
    Some(expansion)
}

fn expansion_text_for_range(
    model: &SourcePreprocModel,
    emitted_range: SourceEmittedTokenRange,
) -> Option<String> {
    let mut text = String::new();
    let end = emitted_range.start.raw().checked_add(emitted_range.len)?;
    for raw in emitted_range.start.raw()..end {
        let token = model.emitted_tokens().get(SourceEmittedTokenId::new(raw))?;
        text.push_str(token.display.as_str());
    }
    Some(text)
}
