use std::{fmt, ops::Range};

use ::preproc::source::PreprocSourceId;
use rustc_hash::FxHashSet;
use syntax::{
    SourceBufferRange,
    ast::{AstNode, CompilationUnit, Member},
    preproc::{MacroCallId as TraceMacroCallId, MacroDefinitionId, MacroExpansionId, TokenOrigin},
};
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    paths::{AbsPathBuf, Utf8PathBuf},
};
use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

use super::*;
use crate::{
    base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        salsa::{self, Durability},
        source_db::{
            FileLoader, PreprocSourceMap, SourceDb, SourceDbStorage, SourceFileKind,
            SourcePreprocQueryError, SourceRootDb, SourceRootDbStorage,
        },
        source_root::{SourceRoot, SourceRootId},
    },
    db::{HirDb, HirDbStorage, InternDb, InternDbStorage},
    file::HirFileId,
};

const TOP: FileId = FileId::from_raw(0);
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
        let source_root_id = SourceRootDb::source_root_id(self, path.anchor);
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
    db
}

fn abs_path(path: &str) -> AbsPathBuf {
    let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
    AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
}

fn text_at_range(text: &str, range: TextRange) -> &str {
    &text[usize::from(range.start())..usize::from(range.end())]
}

fn offset(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).expect("needle should exist")).unwrap())
}

fn range(buffer_id: u32, range: Range<usize>) -> SourceBufferRange {
    SourceBufferRange { buffer_id, range }
}

fn text_range(start: u32, end: u32) -> TextRange {
    TextRange::new(TextSize::from(start), TextSize::from(end))
}

fn test_macro_call(db: &TestDb, trace_call: TraceMacroCallId) -> MacroCallId {
    db.intern_macro_call(MacroCallLoc { model_file: TOP, trace_call })
}

#[test]
fn expansion_source_map_maps_trace_origins_and_missing_slots() {
    let db = TestDb::default();
    let mut preproc_source_map = PreprocSourceMap::default();
    preproc_source_map.insert_real_file(PreprocSourceId::from(7), TOP, 64);
    let body_call = test_macro_call(&db, TraceMacroCallId(11));
    let arg_call = test_macro_call(&db, TraceMacroCallId(21));
    let operation_call = test_macro_call(&db, TraceMacroCallId(31));
    let origins = vec![
        TokenOrigin::Source { token_range: range(7, 1..4) },
        TokenOrigin::MacroBody {
            macro_name: "BODY".to_owned(),
            call_id: TraceMacroCallId(11),
            definition_id: MacroDefinitionId(12),
            expansion_id: MacroExpansionId(13),
            parent_expansion_id: None,
            body_token_index: 0,
            call_range: range(7, 10..15),
            body_token_range: range(7, 20..24),
        },
        TokenOrigin::MacroArgument {
            macro_name: "ARG".to_owned(),
            call_id: TraceMacroCallId(21),
            definition_id: MacroDefinitionId(22),
            expansion_id: MacroExpansionId(23),
            parent_expansion_id: None,
            body_token_index: 0,
            argument_index: 2,
            argument_token_index: 0,
            call_range: range(7, 30..35),
            body_token_range: range(7, 40..44),
            argument_token_range: range(7, 50..54),
        },
        TokenOrigin::TokenPaste {
            call_id: TraceMacroCallId(31),
            definition_id: MacroDefinitionId(32),
            expansion_id: MacroExpansionId(33),
            parent_expansion_id: None,
            body_token_index: 0,
            argument_index: None,
            argument_token_index: None,
        },
        TokenOrigin::Unavailable,
    ];

    let source_map =
        ExpansionSourceMap::from_token_origins(&db, TOP, &origins, &preproc_source_map);

    assert_eq!(source_map.map_up(0), Some(Origin::File { file: TOP, range: text_range(1, 4) }));
    assert_eq!(
        source_map.map_up(1),
        Some(Origin::MacroBody {
            call: body_call,
            def: MacroDefinitionId(12),
            body_range: text_range(20, 24),
        })
    );
    assert_eq!(
        source_map.map_up(2),
        Some(Origin::MacroArg { call: arg_call, arg_index: 2, arg_range: text_range(50, 54) })
    );
    assert_eq!(source_map.map_up(3), Some(Origin::TokenPaste { call: operation_call }));
    assert_eq!(source_map.map_up(4), None);
    assert_eq!(source_map.map_down(&Origin::TokenPaste { call: operation_call }), vec![3]);
    assert!(source_map.map_down(&Origin::Stringify { call: operation_call }).is_empty());
    assert_eq!(
        source_map.source_hits(TOP, TextSize::from(21)),
        vec![ExpansionSourceHit {
            emitted_token: SourceEmittedTokenId::new(1),
            expanded_token_index: 1,
            range: text_range(20, 24),
            origin: Origin::MacroBody {
                call: body_call,
                def: MacroDefinitionId(12),
                body_range: text_range(20, 24),
            },
        }]
    );
    assert_eq!(
        source_map.source_hits(TOP, TextSize::from(51)),
        vec![ExpansionSourceHit {
            emitted_token: SourceEmittedTokenId::new(2),
            expanded_token_index: 2,
            range: text_range(50, 54),
            origin: Origin::MacroArg {
                call: arg_call,
                arg_index: 2,
                arg_range: text_range(50, 54),
            },
        }]
    );
    assert!(source_map.source_hits(TOP, TextSize::from(70)).is_empty());
}

#[test]
fn macro_file_expansion_parses_emitted_tokens_and_maps_origins() {
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

    let macro_call = db.intern_macro_call(MacroCallLoc {
        model_file: TOP,
        trace_call: call.trace_call.expect("macro call should carry slang trace identity"),
    });
    let macro_file = db.intern_macro_file(MacroFileLoc { call: macro_call });
    let expansion = db.macro_expansion(macro_file);

    assert!(expansion.value.text.contains("module"));
    assert!(expansion.value.text.contains("from_macro"));
    assert!(matches!(expansion.value.source_map.map_up(0), Some(Origin::MacroBody { .. })));
    let parse = db.parse(HirFileId::Macro(macro_file));
    let root = parse.root().expect("macro expansion should parse to a syntax root");
    let unit =
        CompilationUnit::cast(root).expect("macro expansion root should be a compilation unit");
    let mut modules = unit.members().children().filter_map(Member::as_module_declaration);
    let module = modules.next().expect("macro expansion should contain a module");
    assert!(modules.next().is_none());
    assert_eq!(module.header().name().unwrap().value_text().to_string(), "from_macro");
}

#[test]
fn macro_files_at_offset_returns_available_expansions() {
    let root_text = "`define DECL module from_macro; endmodule\n`DECL\n";
    let db = db_with_root_text(root_text);

    let macro_files = macro_files_at_offset(&db, TOP, offset(root_text, "`DECL"));

    assert_eq!(macro_files.len(), 1);
    let macro_file_loc = db.lookup_intern_macro_file(macro_files[0]);
    let macro_call_loc = db.lookup_intern_macro_call(macro_file_loc.call);
    assert_eq!(macro_call_loc.model_file, TOP);
    let expansion = db.macro_expansion(macro_files[0]);
    assert!(expansion.value.text.contains("from_macro"));
}

#[test]
fn macro_expansion_reports_missing_trace_call() {
    let db = db_with_root_text("`define EMPTY\n`EMPTY\n");
    let trace_call = TraceMacroCallId(u32::MAX);
    let macro_call = db.intern_macro_call(MacroCallLoc { model_file: TOP, trace_call });
    let macro_file = db.intern_macro_file(MacroFileLoc { call: macro_call });

    let expansion = db.macro_expansion(macro_file);

    assert_eq!(expansion.value.text, "");
    assert_eq!(
        expansion.err.as_ref().map(ExpandError::kind),
        Some(&ExpandErrorKind::MissingTraceCall { trace_call })
    );
}

#[test]
fn macro_expansion_reports_preproc_model_failure() {
    let mut db = db_with_root_text("`define EMPTY\n`EMPTY\n");
    db.set_file_kind_with_durability(TOP, SourceFileKind::LibraryMap, Durability::LOW);
    let trace_call = TraceMacroCallId(0);
    let macro_call = db.intern_macro_call(MacroCallLoc { model_file: TOP, trace_call });
    let macro_file = db.intern_macro_file(MacroFileLoc { call: macro_call });

    let expansion = db.macro_expansion(macro_file);

    assert_eq!(expansion.value.text, "");
    assert_eq!(
        expansion.err.as_ref().map(ExpandError::kind),
        Some(&ExpandErrorKind::SourcePreprocModel(SourcePreprocQueryError::UnsupportedFileKind(
            SourceFileKind::LibraryMap
        )))
    );
}

#[test]
fn expansion_text_reports_missing_emitted_token() {
    let db = db_with_root_text("`define ONE 1\n`ONE\n");
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().expect("preproc model should be available");
    let missing = SourceEmittedTokenId::new(mapped.model.emitted_tokens().len());

    let expansion =
        expansion_text_for_range(&mapped.model, SourceEmittedTokenRange { start: missing, len: 1 });

    assert_eq!(expansion.value, "");
    assert_eq!(
        expansion.err.as_ref().map(ExpandError::kind),
        Some(&ExpandErrorKind::MissingEmittedToken { token: missing })
    );
}

#[test]
fn expansion_source_map_reports_missing_trace_token() {
    let db = db_with_root_text("`define ONE 1\n`ONE\n");
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().expect("preproc model should be available");
    let parsed = db.parsed_compilation_unit(TOP);
    let trace = parsed.preprocessor_trace.as_ref().expect("preprocessor trace should be available");
    let missing = SourceEmittedTokenId::new(trace.emitted_tokens.len());

    let expansion = ExpansionSourceMap::from_trace_range(
        &db,
        TOP,
        trace,
        &mapped.source_map,
        SourceEmittedTokenRange { start: missing, len: 1 },
    );

    assert!(expansion.value.is_empty());
    assert_eq!(
        expansion.err.as_ref().map(ExpandError::kind),
        Some(&ExpandErrorKind::SourceMap(ExpansionSourceMapError::MissingTraceToken {
            token: missing,
        }))
    );
}

#[test]
fn expansion_text_validates_zero_length_range_start() {
    let db = db_with_root_text("`define EMPTY\n`EMPTY\n");
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().expect("preproc model should be available");
    let table_len = mapped.model.emitted_tokens().len();
    let valid_start = SourceEmittedTokenId::new(table_len);

    let valid = expansion_text_for_range(
        &mapped.model,
        SourceEmittedTokenRange { start: valid_start, len: 0 },
    );

    assert_eq!(valid, ExpandResult::ok(String::new()));

    let invalid_start = SourceEmittedTokenId::new(table_len + 1);
    let invalid = expansion_text_for_range(
        &mapped.model,
        SourceEmittedTokenRange { start: invalid_start, len: 0 },
    );

    assert_eq!(invalid.value, "");
    assert_eq!(
        invalid.err.as_ref().map(ExpandError::kind),
        Some(&ExpandErrorKind::InvalidEmittedTokenRange { start: invalid_start, len: 0 })
    );
}

#[test]
fn expansion_source_map_validates_zero_length_range_start() {
    let db = db_with_root_text("`define EMPTY\n`EMPTY\n");
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().expect("preproc model should be available");
    let parsed = db.parsed_compilation_unit(TOP);
    let trace = parsed.preprocessor_trace.as_ref().expect("preprocessor trace should be available");
    let table_len = trace.emitted_tokens.len();
    let valid_start = SourceEmittedTokenId::new(table_len);

    let valid = ExpansionSourceMap::from_trace_range(
        &db,
        TOP,
        trace,
        &mapped.source_map,
        SourceEmittedTokenRange { start: valid_start, len: 0 },
    );

    assert_eq!(valid, ExpandResult::ok(ExpansionSourceMap::empty()));

    let invalid_start = SourceEmittedTokenId::new(table_len + 1);
    let invalid = ExpansionSourceMap::from_trace_range(
        &db,
        TOP,
        trace,
        &mapped.source_map,
        SourceEmittedTokenRange { start: invalid_start, len: 0 },
    );

    assert_eq!(invalid.value, ExpansionSourceMap::empty());
    assert_eq!(
        invalid.err.as_ref().map(ExpandError::kind),
        Some(&ExpandErrorKind::SourceMap(ExpansionSourceMapError::InvalidEmittedTokenRange {
            start: invalid_start,
            len: 0,
        }))
    );
}

#[test]
fn expansion_source_map_preserves_valid_prefix_before_missing_trace_token() {
    let db = db_with_root_text("`define ONE 1\n`ONE\n");
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().expect("preproc model should be available");
    let parsed = db.parsed_compilation_unit(TOP);
    let trace = parsed.preprocessor_trace.as_ref().expect("preprocessor trace should be available");
    let table_len = trace.emitted_tokens.len();
    assert!(table_len > 0, "fixture should emit at least one token");
    let missing = SourceEmittedTokenId::new(table_len);

    let expansion = ExpansionSourceMap::from_trace_range(
        &db,
        TOP,
        trace,
        &mapped.source_map,
        SourceEmittedTokenRange { start: SourceEmittedTokenId::new(0), len: table_len + 1 },
    );

    assert!(!expansion.value.is_empty());
    assert_eq!(
        expansion.err.as_ref().map(ExpandError::kind),
        Some(&ExpandErrorKind::SourceMap(ExpansionSourceMapError::MissingTraceToken {
            token: missing,
        }))
    );
}

#[test]
fn expansion_info_preserves_source_map_when_text_extraction_fails() {
    let db = db_with_root_text("`define ONE 1\n`ONE\n");
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().expect("preproc model should be available");
    let parsed = db.parsed_compilation_unit(TOP);
    let trace = parsed.preprocessor_trace.as_ref().expect("preprocessor trace should be available");
    assert!(!trace.emitted_tokens.is_empty(), "fixture should emit at least one token");
    let source_map = ExpansionSourceMap::from_trace_range(
        &db,
        TOP,
        trace,
        &mapped.source_map,
        SourceEmittedTokenRange { start: SourceEmittedTokenId::new(0), len: 1 },
    );
    let missing = SourceEmittedTokenId::new(mapped.model.emitted_tokens().len());
    let text = ExpandResult::new(
        String::new(),
        ExpandError::new(ExpandErrorKind::MissingEmittedToken { token: missing }),
    );

    let expansion = expansion_info_from_parts(text, source_map);

    assert!(!expansion.value.source_map.is_empty());
    assert_eq!(
        expansion.err.as_ref().map(ExpandError::kind),
        Some(&ExpandErrorKind::MissingEmittedToken { token: missing })
    );
}
