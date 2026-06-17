use ::preproc::source::{
    SourceEmittedTokenId, SourceEmittedTokenRange, SourceMacroCallId, SourceMacroExpansionQuery,
    SourcePreprocModel,
};
use syntax::SyntaxTree;
use triomphe::Arc;
use vfs::FileId;

use crate::{base_db::salsa, db::HirDb};

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExpansionSourceMap {
    origins: Vec<()>,
}

impl ExpansionSourceMap {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }
}

pub(crate) fn macro_expansion_query(db: &dyn HirDb, macro_file: MacroFileId) -> Arc<ExpansionInfo> {
    let loc = db.lookup_intern_macro_file(macro_file);
    let text = db
        .source_preproc_model(loc.model_file)
        .as_ref()
        .as_ref()
        .ok()
        .and_then(|mapped| expansion_text_for_call(&mapped.model, loc.call))
        .unwrap_or_default();
    let parse = SyntaxTree::from_text(&text, "macro-expansion", "");
    Arc::new(ExpansionInfo { text, parse, source_map: ExpansionSourceMap::empty() })
}

fn expansion_text_for_call(model: &SourcePreprocModel, call: SourceMacroCallId) -> Option<String> {
    let expansion = match model.immediate_macro_expansion(call) {
        SourceMacroExpansionQuery::Available(expansion) => {
            model.macro_expansions().get(expansion)?
        }
        SourceMacroExpansionQuery::Unavailable(_) => return None,
    };
    expansion_text_for_range(model, expansion.emitted_token_range)
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

#[cfg(test)]
mod tests {
    use std::fmt;

    use rustc_hash::FxHashSet;
    use syntax::ast::{AstNode, CompilationUnit, Member};
    use triomphe::Arc;
    use utils::{
        line_index::TextRange,
        paths::{AbsPathBuf, Utf8PathBuf},
    };
    use vfs::{FileSet, VfsPath, anchored_path::AnchoredPath};

    use super::*;
    use crate::{
        base_db::{
            diagnostics_config::DiagnosticsConfig,
            project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
            salsa::{self, Durability},
            source_db::{
                FileLoader, SourceDb, SourceDbStorage, SourceFileKind, SourceRootDb,
                SourceRootDbStorage,
            },
            source_root::{SourceRoot, SourceRootId},
        },
        db::{HirDb, HirDbStorage, InternDb, InternDbStorage},
        file::HirFileId,
    };

    const TOP: FileId = FileId(0);
    const ROOT: SourceRootId = SourceRootId(0);
    const PROFILE: CompilationProfileId = CompilationProfileId(0);

    #[salsa::database(SourceDbStorage, SourceRootDbStorage, InternDbStorage, HirDbStorage)]
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
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor_id);
            SourceRootDb::source_root(self, source_root_id).resolve_path(path)
        }
    }

    fn db_with_root_text(root_text: &str) -> TestDb {
        let top_path = abs_path("rtl/top.v");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);
        let mut files = FxHashSet::default();
        files.insert(TOP);

        let preprocess = PreprocessConfig::default();
        let project_config = ProjectConfig::new(
            vec![Some(PROFILE)],
            vec![CompilationProfile {
                source_roots: vec![ROOT],
                top_modules: Vec::new(),
                preprocess: preprocess.clone(),
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
        db.set_file_kind_with_durability(TOP, SourceFileKind::SystemVerilog, Durability::LOW);
        db.set_file_text_with_durability(TOP, Arc::from(root_text), Durability::LOW);
        db.set_file_preprocess_config_with_durability(TOP, Arc::new(preprocess), Durability::LOW);
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    fn text_at_range(text: &str, range: TextRange) -> &str {
        &text[usize::from(range.start())..usize::from(range.end())]
    }

    #[test]
    fn macro_file_expansion_parses_emitted_tokens() {
        let root_text = "`define DECL module from_macro; endmodule\n`DECL\n";
        let db = db_with_root_text(root_text);
        let mapped = db.source_preproc_model(TOP);
        let mapped = mapped.as_ref().as_ref().expect("preproc model should be available");
        let call = mapped
            .model
            .macro_calls()
            .iter()
            .find(|call| {
                mapped
                    .source_map
                    .map_range(call.call_range)
                    .is_ok_and(|range| text_at_range(root_text, range) == "`DECL")
            })
            .expect("macro call should be recorded");

        let macro_file = db.intern_macro_file(MacroFileLoc { model_file: TOP, call: call.id });
        let expansion = db.macro_expansion(macro_file);

        assert!(expansion.text.contains("module"));
        assert!(expansion.text.contains("from_macro"));
        assert!(expansion.source_map.is_empty());
        let parse = db.parse(HirFileId::Macro(macro_file));
        let root = parse.root().expect("macro expansion should parse to a syntax root");
        let unit =
            CompilationUnit::cast(root).expect("macro expansion root should be a compilation unit");
        let mut modules = unit.members().children().filter_map(Member::as_module_declaration);
        let module = modules.next().expect("macro expansion should contain a module");
        assert!(modules.next().is_none());
        assert_eq!(module.header().name().unwrap().value_text().to_string(), "from_macro");
    }
}
