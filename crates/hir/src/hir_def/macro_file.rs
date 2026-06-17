use ::preproc::source::{
    SourceEmittedTokenId, SourceEmittedTokenRange, SourceMacroCallId, SourceMacroDefinition,
    SourceMacroExpansion, SourceMacroExpansionDefinition, SourceMacroExpansionQuery,
    SourceMacroReferenceId, SourcePreprocModel,
};
use smol_str::SmolStr;
use syntax::SyntaxTree;
use triomphe::Arc;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{base_db::salsa, db::HirDb};

mod source_map;
#[cfg(test)]
mod tests;

pub use source_map::{ExpansionSourceHit, ExpansionSourceMap, Origin};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct MacroFileId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct MacroFileLoc {
    pub model_file: FileId,
    pub call: SourceMacroCallId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionInfo {
    pub text: String,
    pub parse: SyntaxTree,
    pub source_map: ExpansionSourceMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroFileExpansion {
    pub call: MacroFileCall,
    pub definition: MacroFileExpansionDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroFileCall {
    pub reference_id: SourceMacroReferenceId,
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroFileExpansionDefinition {
    Source(MacroFileDefinition),
    Builtin { name: SmolStr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroFileDefinition {
    pub file_id: FileId,
    pub name: SmolStr,
    pub params: Option<Vec<MacroFileDefinitionParam>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroFileDefinitionParam {
    pub name: Option<SmolStr>,
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
            let macro_file = db.intern_macro_file(MacroFileLoc { model_file, call });
            if !macro_files.contains(&macro_file) {
                macro_files.push(macro_file);
            }
        }
    }
    macro_files
}

pub fn macro_file_expansion(db: &dyn HirDb, macro_file: MacroFileId) -> Option<MacroFileExpansion> {
    let loc = db.lookup_intern_macro_file(macro_file);
    let mapped = db.source_preproc_model(loc.model_file);
    let mapped = mapped.as_ref().as_ref().ok()?;
    let call = mapped.model.macro_calls().get(loc.call)?;
    let expansion = source_expansion_for_call(&mapped.model, loc.call)?;
    Some(MacroFileExpansion {
        call: MacroFileCall {
            reference_id: call.reference,
            file_id: mapped.source_map.file_id(call.call_range.source).ok()?,
            range: mapped.source_map.map_range(call.call_range).ok()?,
        },
        definition: macro_file_expansion_definition(mapped, expansion)?,
    })
}

pub(crate) fn macro_expansion_query(db: &dyn HirDb, macro_file: MacroFileId) -> Arc<ExpansionInfo> {
    let loc = db.lookup_intern_macro_file(macro_file);
    let mapped = db.source_preproc_model(loc.model_file);
    let expansion = mapped.as_ref().as_ref().ok().and_then(|mapped| {
        emitted_range_for_call(&mapped.model, loc.call).map(|range| (mapped, range))
    });
    let (text, source_map) = expansion
        .map(|(mapped, emitted_range)| {
            let text = expansion_text_for_range(&mapped.model, emitted_range).unwrap_or_default();
            let source_map = db
                .parsed_compilation_unit(loc.model_file)
                .preprocessor_trace
                .as_ref()
                .map(|trace| {
                    ExpansionSourceMap::from_trace_range(trace, &mapped.source_map, emitted_range)
                })
                .unwrap_or_else(ExpansionSourceMap::empty);
            (text, source_map)
        })
        .unwrap_or_else(|| (String::new(), ExpansionSourceMap::empty()));
    let parse = SyntaxTree::from_text(&text, "macro-expansion", "");
    Arc::new(ExpansionInfo { text, parse, source_map })
}

fn macro_file_expansion_definition(
    mapped: &crate::base_db::source_db::MappedSourcePreprocModel,
    expansion: &SourceMacroExpansion,
) -> Option<MacroFileExpansionDefinition> {
    match &expansion.definition {
        SourceMacroExpansionDefinition::Source(definition) => mapped
            .model
            .macro_definitions()
            .get(*definition)
            .and_then(|definition| macro_file_definition(mapped, definition))
            .map(MacroFileExpansionDefinition::Source),
        SourceMacroExpansionDefinition::Builtin { name } => {
            Some(MacroFileExpansionDefinition::Builtin { name: name.clone() })
        }
    }
}

fn macro_file_definition(
    mapped: &crate::base_db::source_db::MappedSourcePreprocModel,
    definition: &SourceMacroDefinition,
) -> Option<MacroFileDefinition> {
    let file_id = mapped
        .source_map
        .predefine_manifest_source(definition.name_range.source)
        .map(|source| source.file_id)
        .or_else(|| mapped.source_map.file_id(definition.name_range.source).ok())?;
    let params = definition.params.as_ref().map(|params| {
        params.iter().map(|param| MacroFileDefinitionParam { name: param.name.clone() }).collect()
    });
    Some(MacroFileDefinition { file_id, name: definition.name.clone(), params })
}

fn emitted_range_for_call(
    model: &SourcePreprocModel,
    call: SourceMacroCallId,
) -> Option<SourceEmittedTokenRange> {
    source_expansion_for_call(model, call).map(|expansion| expansion.emitted_token_range)
}

fn source_expansion_for_call(
    model: &SourcePreprocModel,
    call: SourceMacroCallId,
) -> Option<&SourceMacroExpansion> {
    let expansion = match model.immediate_macro_expansion(call) {
        SourceMacroExpansionQuery::Available(expansion) => {
            model.macro_expansions().get(expansion)?
        }
        SourceMacroExpansionQuery::Unavailable(_) => return None,
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
