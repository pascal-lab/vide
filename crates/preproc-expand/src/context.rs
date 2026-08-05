//! Per-file macro invocation coverage.
//!
//! Replaces the IDE-side text heuristic
//! (`source_macro_invocation_may_cover_offset`) with an indexed protocol: one
//! salsa query per file builds the set of macro call spans mapped into that
//! file (from every compilation context that includes it), and
//! `macro_context_at` answers "which macro expansions could own this offset"
//! with a binary search instead of a backwards text scan per token.

use triomphe::Arc;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    db::PreprocDb,
    macro_file::{MacroCallLoc, MacroFileId},
    source_db::range_index::RangeIndex,
};

/// The macro state at one source offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroContext {
    /// No macro invocation covers the offset; callers fall back to ordinary
    /// syntax resolution.
    NoInvocation,
    /// One or more macro invocations cover the offset. `macro_files` are the
    /// expansions that could own it, in source order; callers still have to
    /// consult each expansion's source map for the exact token mapping.
    Invocation { macro_files: Vec<MacroFileId> },
}

/// Coverage spans of one file, sorted and indexed for offset queries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacroCoverage {
    spans: RangeIndex<MacroFileId>,
}

impl MacroCoverage {
    pub(crate) fn push(&mut self, range: TextRange, macro_file: MacroFileId) {
        self.spans.push(range, macro_file);
    }

    pub(crate) fn finish(&mut self) {
        self.spans.finish();
    }

    fn macro_files_at(&self, offset: TextSize) -> Vec<MacroFileId> {
        self.spans.ids_at(offset)
    }
}

pub fn macro_context_at(db: &dyn PreprocDb, file_id: FileId, offset: TextSize) -> MacroContext {
    let macro_files = db.file_macro_coverage(file_id).macro_files_at(offset);
    if macro_files.is_empty() {
        MacroContext::NoInvocation
    } else {
        MacroContext::Invocation { macro_files }
    }
}

pub(crate) fn file_macro_coverage_query(db: &dyn PreprocDb, file_id: FileId) -> Arc<MacroCoverage> {
    let mut model_file_ids = vec![file_id];
    for model_file_id in &db.source_preproc_contexts_for_file(file_id).model_file_ids {
        if !model_file_ids.contains(model_file_id) {
            model_file_ids.push(*model_file_id);
        }
    }

    let mut coverage = MacroCoverage::default();
    for model_file in model_file_ids {
        let model = db.source_preproc_model(model_file);
        let Ok(mapped) = model.as_ref().as_ref() else {
            continue;
        };
        let parsed = db.parsed_compilation_unit(model_file);
        if parsed.preprocessor_trace.is_none() {
            continue;
        }
        let trace_index = db.trace_index(model_file);
        for call in mapped.model.macro_calls().iter() {
            let Some(trace_call) = call.trace_call else {
                continue;
            };
            if trace_index.emitted_range_for_call(trace_call).is_none() {
                continue;
            }
            let Ok(call_file) = mapped.source_map.file_id(call.call_range.source) else {
                continue;
            };
            if call_file != file_id {
                continue;
            }
            let Ok(range) = mapped.source_map.map_range(call.call_range) else {
                continue;
            };
            let macro_file = db.intern_macro_file(MacroCallLoc { model_file, trace_call });
            coverage.push(range, macro_file);
        }
    }
    coverage.finish();

    Arc::new(coverage)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fmt};

    use base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        salsa::{self, Durability},
        source_db::{FileLoader, SourceDb, SourceDbStorage, SourceRootDb, SourceRootDbStorage},
        source_root::{SourceRoot, SourceRootId},
    };
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use super::*;
    use crate::db::PreprocDbStorage;

    const TOP: FileId = FileId::from_raw(0);
    const ROOT: SourceRootId = SourceRootId(0);
    const PROFILE: CompilationProfileId = CompilationProfileId(0);

    #[salsa::database(SourceDbStorage, SourceRootDbStorage, PreprocDbStorage)]
    #[derive(Default)]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    impl salsa::Database for TestDb {}

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl FileLoader for TestDb {
        fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor);
            SourceRootDb::source_root(self, source_root_id).resolve_path(path)
        }
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    fn db_with_root_text(root_text: &str) -> TestDb {
        let top_path = abs_path("rtl/top.v");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);
        let mut files = HashSet::default();
        files.insert(TOP);

        let project_config = ProjectConfig::new(
            vec![Some(PROFILE)],
            vec![CompilationProfile {
                source_roots: vec![ROOT],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig::default(),
            }],
        );

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_project_config_with_durability(Arc::new(project_config), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        db.set_source_root_id_with_durability(TOP, ROOT, Durability::LOW);
        db.set_file_path_with_durability(TOP, Some(top_path), Durability::LOW);
        db.set_file_kind_with_durability(
            TOP,
            base_db::source_db::SourceFileKind::SystemVerilog,
            Durability::LOW,
        );
        db.set_file_text_with_durability(TOP, Arc::from(root_text), Durability::LOW);
        db
    }

    fn offset(text: &str, needle: &str) -> TextSize {
        TextSize::from(u32::try_from(text.find(needle).expect("needle should exist")).unwrap())
    }

    #[test]
    fn plain_identifier_has_no_invocation_context() {
        let text = "module m; wire payload_i; endmodule\n";
        let db = db_with_root_text(text);
        assert_eq!(
            macro_context_at(&db, TOP, offset(text, "payload_i")),
            MacroContext::NoInvocation
        );
    }

    #[test]
    fn macro_name_and_arguments_are_covered() {
        let text = "`define MAKE_DECL(x) module m; wire x; endmodule\n`MAKE_DECL(payload_i)\n";
        let db = db_with_root_text(text);
        for needle in ["`MAKE_DECL", "payload_i"] {
            let MacroContext::Invocation { macro_files } =
                macro_context_at(&db, TOP, offset(text, needle))
            else {
                panic!("expected invocation context at {needle:?}");
            };
            assert_eq!(macro_files.len(), 1);
        }
    }

    #[test]
    fn outer_arguments_after_nested_macros_are_covered() {
        let text =
            "`define OUTER(a, b) a + b\n`define INNER(x) x\nassign y = `OUTER(`INNER(b), c);\n";
        let db = db_with_root_text(text);
        // The trailing argument follows a nested invocation but still belongs
        // to the outer call's span.
        let MacroContext::Invocation { macro_files } =
            macro_context_at(&db, TOP, offset(text, "c"))
        else {
            panic!("expected invocation context");
        };
        assert_eq!(macro_files.len(), 1, "only the outer call owns the argument tail");
    }

    #[test]
    fn nested_invocations_report_both_expansions() {
        let text = "`define OUTER(a) a\n`define INNER(x) x\nassign y = `OUTER(`INNER(b));\n";
        let db = db_with_root_text(text);
        let MacroContext::Invocation { macro_files } =
            macro_context_at(&db, TOP, offset(text, "`INNER"))
        else {
            panic!("expected invocation context");
        };
        assert_eq!(macro_files.len(), 2, "both outer and inner expansions cover the offset");
    }
}
