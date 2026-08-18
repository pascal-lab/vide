//! Syntactic name occurrence table.
//!
//! The workspace product for find-references is "which files mention this
//! identifier text", not "every identifier resolved to a `DefId`". Resolution
//! happens on demand, only for occurrences of the name being searched.

use preproc_expand::macro_file::SourceEmittedTokenId;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use syntax::TokenKind;
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::analysis::AnalysisContext;

mod build;

/// One name-like CST token, recorded without resolving it.
///
/// `emitted` is the preprocessor-trace identity when the token has one.
/// Macro-expanded trees share display ranges across body tokens, so
/// `token_at_offset` cannot recover those tokens; the emitted id can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NameOccurrence {
    pub range: TextRange,
    pub kind: TokenKind,
    pub emitted: Option<SourceEmittedTokenId>,
}

/// Per-file slice: identifier text to the tokens that spell it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileNameIndex {
    occurrences: FxHashMap<SmolStr, Box<[NameOccurrence]>>,
}

impl FileNameIndex {
    pub(crate) fn for_file(
        db: &dyn crate::db::workspace_symbol_index_db::WorkspaceSymbolIndexDb,
        file_id: FileId,
    ) -> Self {
        build::collect_file(db, file_id)
    }

    pub(crate) fn occurrences(&self, name: &str) -> &[NameOccurrence] {
        self.occurrences.get(name).map_or(&[], |occurrences| occurrences.as_ref())
    }

    fn names(&self) -> impl Iterator<Item = &SmolStr> {
        self.occurrences.keys()
    }
}

/// Merged name → files map for one source root, plus the per-file tables.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameIndex {
    files_by_name: FxHashMap<SmolStr, Box<[FileId]>>,
    files: FxHashMap<FileId, Arc<FileNameIndex>>,
}

impl NameIndex {
    pub(crate) fn from_file_indexes(file_indexes: &FxHashMap<FileId, Arc<FileNameIndex>>) -> Self {
        let mut files_by_name: FxHashMap<SmolStr, Vec<FileId>> = FxHashMap::default();
        for (&file_id, index) in file_indexes {
            for name in index.names() {
                files_by_name.entry(name.clone()).or_default().push(file_id);
            }
        }
        for files in files_by_name.values_mut() {
            files.sort_by_key(|file_id| file_id.index());
            files.dedup();
        }
        Self {
            files_by_name: files_by_name
                .into_iter()
                .map(|(name, files)| (name, files.into_boxed_slice()))
                .collect(),
            files: file_indexes.clone(),
        }
    }

    pub(crate) fn files_mentioning(&self, name: &str) -> &[FileId] {
        self.files_by_name.get(name).map_or(&[], |files| files.as_ref())
    }
}

/// Compilation-unit files that belong in the name table for `source_root_id`.
///
/// This is the `vide.toml` / profile source set (`CompilationPlan::roots`),
/// not every path in the VFS source root.
pub(crate) fn index_files_for_root(
    ctx: &AnalysisContext<'_>,
    source_root_id: base_db::source_root::SourceRootId,
) -> Vec<FileId> {
    let plan = ctx.compilation_plan_for_root(source_root_id);
    let mut files: Vec<FileId> = plan
        .all_file_ids()
        .into_iter()
        .filter(|&file_id| ctx.source_root_id(file_id) == source_root_id)
        .collect();
    files.sort_by_key(|file_id| file_id.index());
    files.dedup();
    files
}

#[cfg(test)]
mod tests {
    use syntax::{has_text_range::HasTextRange, token::TokenKindExt};
    use utils::line_index::TextSize;

    use super::FileNameIndex;
    use crate::{semantic_target::preproc::emit_token_index, test_utils::setup_marked};

    #[test]
    fn macro_argument_occurrence_recovers_via_emitted_id() {
        let text = r#"
`define NEXT(value) (value + 1)
module top(input logic /*marker:def*/payload_i);
  logic active_data;
  assign active_data = `NEXT(/*marker:arg*/payload_i);
endmodule
"#;
        let (host, file_id, _clean, markers) = setup_marked(text);
        let db = host.ctx();
        let arg = utils::line_index::TextRange::new(
            markers["arg"],
            markers["arg"] + TextSize::of("payload_i"),
        );
        let index = FileNameIndex::for_file(db.db, file_id);
        let occurrence = index
            .occurrences("payload_i")
            .iter()
            .find(|occurrence| occurrence.range == arg)
            .expect("CST walk records the macro argument identifier");
        assert!(occurrence.emitted.is_some(), "macro-argument tokens have a trace identity");

        let tree = db.parse(preproc_expand::file::HirFileId::from(file_id));
        let emitted = emit_token_index(tree.root());
        let token = crate::references::search::token_for_occurrence(&tree, &emitted, occurrence)
            .expect("emitted-id lookup recovers the argument token");
        assert!(token.kind().name_like());
        assert_eq!(token.text_range(), Some(arg));
        assert_eq!(token.raw_text(), "payload_i");
    }

    #[test]
    fn design_unit_name_range_covers_the_declaration_token() {
        let text = "module /*marker:name*/top; endmodule\n";
        let (host, file_id, _clean, markers) = setup_marked(text);
        let decl = host
            .ctx()
            .file_facts(file_id)
            .design_unit_at(markers["name"])
            .expect("file facts record the module name")
            .clone();
        assert_eq!(decl.id.name, "top");
        assert_eq!(decl.id.kind, design_graph::UnitKind::Module);
        assert_eq!(
            decl.name_range,
            Some(utils::line_index::TextRange::new(
                markers["name"],
                markers["name"] + TextSize::of("top"),
            ))
        );
    }
}
