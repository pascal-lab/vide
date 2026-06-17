use ::preproc::source::{
    SourceEmittedTokenId, SourceEmittedTokenRange, SourceMacroCallId, SourceMacroExpansionQuery,
    SourcePreprocModel,
};
use syntax::SyntaxTree;
use triomphe::Arc;
use vfs::FileId;

use crate::{base_db::salsa, db::HirDb};

mod source_map;
#[cfg(test)]
mod tests;

pub use source_map::{ExpansionSourceMap, Origin};

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

fn emitted_range_for_call(
    model: &SourcePreprocModel,
    call: SourceMacroCallId,
) -> Option<SourceEmittedTokenRange> {
    let expansion = match model.immediate_macro_expansion(call) {
        SourceMacroExpansionQuery::Available(expansion) => {
            model.macro_expansions().get(expansion)?
        }
        SourceMacroExpansionQuery::Unavailable(_) => return None,
    };
    Some(expansion.emitted_token_range)
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
