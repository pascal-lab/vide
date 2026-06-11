use std::fmt;

use rustc_hash::FxHashSet;
use source_model::{
    FilePosition, FileRange, ResolutionReason, ResolvedSourceTarget, SourceEntity, SourceOrigin,
    SourcePurpose, SourceRangeResult, SourceTarget, SourceTargetResolution,
};
use triomphe::Arc;
use utils::{
    get::Get,
    line_index::{TextRange, TextSize},
    paths::{AbsPathBuf, Utf8PathBuf},
};
use vfs::{FileId, FileSet, VfsPath, anchored_path::AnchoredPath};

use super::*;
use crate::{
    base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{
            CompilationProfile, CompilationProfileId, Predefine, PredefineSource, PreprocessConfig,
            ProjectConfig,
        },
        salsa::{self, Durability},
        source_db::{
            FileLoader, SourceDb, SourceDbStorage, SourceFileKind, SourceRootDb,
            SourceRootDbStorage,
        },
        source_root::{SourceRoot, SourceRootId},
    },
    container::InFile,
    db::{HirDb, HirDbStorage, InternDbStorage},
    file::HirFileId,
    hir_def::module::ModuleId,
    source_map::IsSrc,
    source_resolver::PositionResolver,
};

const TOP: FileId = FileId(0);
const HEADER: FileId = FileId(1);
const LEAF: FileId = FileId(2);
const MANIFEST: FileId = FileId(3);
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

fn db_with_files(root_text: &str, header_text: &str) -> TestDb {
    db_with_entries(&[(TOP, "rtl/top.v", root_text), (HEADER, "include/defs.vh", header_text)])
}

fn db_with_nested_files(root_text: &str, header_text: &str, leaf_text: &str) -> TestDb {
    db_with_entries(&[
        (TOP, "rtl/top.v", root_text),
        (HEADER, "include/defs.vh", header_text),
        (LEAF, "include/leaf.vh", leaf_text),
    ])
}

fn db_with_entries(entries: &[(FileId, &str, &str)]) -> TestDb {
    db_with_entries_and_predefines(entries, Vec::new())
}

fn db_with_entries_and_predefines(
    entries: &[(FileId, &str, &str)],
    predefines: Vec<String>,
) -> TestDb {
    db_with_entries_and_predefine_entries(
        entries,
        predefines.into_iter().map(Predefine::new).collect(),
    )
}

fn db_with_entries_and_predefine_entries(
    entries: &[(FileId, &str, &str)],
    predefines: Vec<Predefine>,
) -> TestDb {
    let include_dir = abs_path("include");

    let mut file_set = FileSet::default();
    for (file_id, path, _) in entries {
        file_set.insert(*file_id, VfsPath::from(abs_path(path)));
    }
    let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);

    let preprocess = PreprocessConfig { predefines, include_dirs: vec![include_dir.clone()] };
    let project_config = ProjectConfig::new(
        vec![Some(PROFILE)],
        vec![CompilationProfile {
            source_roots: vec![ROOT],
            top_modules: Vec::new(),
            preprocess: preprocess.clone(),
        }],
    );

    let mut files = FxHashSet::default();
    for (file_id, _, _) in entries {
        files.insert(*file_id);
    }

    let mut db = TestDb::default();
    db.set_files_with_durability(Box::new(files), Durability::HIGH);
    db.set_project_config_with_durability(Arc::new(project_config), Durability::HIGH);
    db.set_diagnostics_config_with_durability(
        Arc::new(DiagnosticsConfig::default()),
        Durability::HIGH,
    );
    db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);

    for (file_id, path, text) in entries {
        let path = abs_path(path);
        let vfs_path = VfsPath::from(path.clone());
        db.set_source_root_id_with_durability(*file_id, ROOT, Durability::LOW);
        db.set_file_path_with_durability(*file_id, Some(path), Durability::LOW);
        db.set_file_kind_with_durability(
            *file_id,
            SourceFileKind::from_path(&vfs_path),
            Durability::LOW,
        );
        db.set_file_text_with_durability(*file_id, Arc::from(*text), Durability::LOW);
        db.set_file_preprocess_config_with_durability(
            *file_id,
            Arc::new(preprocess.clone()),
            Durability::LOW,
        );
    }

    db
}

fn abs_path(path: &str) -> AbsPathBuf {
    let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
    AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
}

fn offset(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).unwrap()).unwrap())
}

fn offset_after(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).unwrap() + needle.len()).unwrap())
}

fn text_at_range(text: &str, range: TextRange) -> &str {
    &text[usize::from(range.start())..usize::from(range.end())]
}

fn source_graph_macro_references_for_definition(
    db: &TestDb,
    model_file_id: FileId,
    definition_file_id: FileId,
    definition_range: TextRange,
) -> Vec<(FileRange, ResolutionReason)> {
    let source_graph = db.source_graph_preproc_model(model_file_id);
    let source_graph = source_graph.as_ref().as_ref().expect("source graph should build");
    let graph = &source_graph.graph;
    let definition = graph
        .entities_intersecting_file_range(definition_file_id, definition_range, None)
        .into_iter()
        .find_map(|hit| {
            let SourceEntity::MacroDefinition(_) = graph.entity(hit.entity) else {
                return None;
            };
            let SourceRangeResult::Mapped(range) =
                graph.entity_focus_file_range(hit.entity, SourcePurpose::FindReferences)
            else {
                return None;
            };
            (range.file_id == definition_file_id && range.range == definition_range)
                .then_some(hit.entity)
        })
        .expect("macro definition should exist in source graph");

    graph
        .resolved_references(source_graph.root_context, definition)
        .iter()
        .filter_map(|(reference, reason)| {
            let SourceEntity::MacroReference(_) = graph.entity(*reference) else {
                return None;
            };
            let SourceRangeResult::Mapped(range) =
                graph.entity_focus_file_range(*reference, SourcePurpose::FindReferences)
            else {
                return None;
            };
            Some((range, *reason))
        })
        .collect()
}

#[test]
fn preproc_include_directive_resolves_literal_target() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let db = db_with_files(root_text, header_text);

    let include = include_directive_at(&db, TOP, offset(root_text, "defs.vh")).unwrap().unwrap();
    assert_eq!(text_at_range(root_text, include.range), "\"defs.vh\"");
    assert!(include_directive_at(&db, TOP, offset(root_text, "`include")).unwrap().is_none());
    assert!(include_directive_at(&db, TOP, include.range.end()).unwrap().is_none());
    let IncludeTarget::Literal { resolved_file, .. } = include.target else {
        panic!("literal include expected");
    };
    assert_eq!(resolved_file, Some(HEADER));
}

#[test]
fn position_resolver_resolves_macro_reference_from_source_graph() {
    let root_text = "`define OBJ 1\nmodule top;\nlocalparam int W = `OBJ;\nendmodule\n";
    let db = db_with_entries(&[(TOP, "rtl/top.sv", root_text)]);

    let resolved = PositionResolver::new(&db).resolve_position(
        FilePosition { file_id: TOP, offset: offset(root_text, "OBJ;") },
        SourcePurpose::GotoDefinition,
        None,
    );

    assert!(matches!(
        resolved,
        SourceTargetResolution::Resolved(ResolvedSourceTarget {
            target: SourceTarget::MacroReference(_),
            ..
        })
    ));
}

#[test]
fn preproc_manifest_predefine_resolves_from_context_source_graph() {
    let root_text = "module top;\nlocalparam int W = `FROM_MANIFEST;\nendmodule\n";
    let manifest_text = "defines = [\"FROM_MANIFEST=1\"]\n";
    let manifest_range = TextRange::new(
        offset(manifest_text, "\"FROM_MANIFEST=1\""),
        offset_after(manifest_text, "\"FROM_MANIFEST=1\""),
    );
    let db = db_with_entries_and_predefine_entries(
        &[(TOP, "rtl/top.sv", root_text), (MANIFEST, "vide.toml", manifest_text)],
        vec![Predefine::with_source(
            "FROM_MANIFEST=1",
            PredefineSource { path: abs_path("vide.toml"), range: manifest_range },
        )],
    );

    let resolved = PositionResolver::new(&db).resolve_position(
        FilePosition { file_id: MANIFEST, offset: offset(manifest_text, "FROM_MANIFEST") },
        SourcePurpose::GotoDefinition,
        None,
    );
    let SourceTargetResolution::Resolved(ResolvedSourceTarget {
        model_file_id,
        entity,
        target: SourceTarget::MacroDefinition(_),
    }) = resolved
    else {
        panic!("manifest predefine should resolve from a context source graph: {resolved:?}");
    };
    assert_eq!(model_file_id, TOP);

    let source_graph = db.source_graph_preproc_model(model_file_id);
    let source_graph = source_graph.as_ref().as_ref().expect("source graph should build");
    assert_eq!(
        source_graph.graph.entity_focus_file_range(entity, SourcePurpose::GotoDefinition),
        SourceRangeResult::Mapped(FileRange { file_id: MANIFEST, range: manifest_range })
    );
}

#[test]
fn position_resolver_resolves_macro_param_targets_from_source_graph() {
    let root_text = "`define SHIFT(value, amount) ((value) << amount)\nmodule top;\nendmodule\n";
    let db = db_with_entries(&[(TOP, "rtl/top.sv", root_text)]);

    let definition = PositionResolver::new(&db).resolve_position(
        FilePosition { file_id: TOP, offset: offset_after(root_text, "SHIFT(") },
        SourcePurpose::GotoDefinition,
        None,
    );
    assert!(matches!(
        definition,
        SourceTargetResolution::Resolved(ResolvedSourceTarget {
            target: SourceTarget::MacroParamDefinition(_),
            ..
        })
    ));

    let reference = PositionResolver::new(&db).resolve_position(
        FilePosition { file_id: TOP, offset: offset(root_text, "value) <<") },
        SourcePurpose::GotoDefinition,
        None,
    );
    assert!(matches!(
        reference,
        SourceTargetResolution::Resolved(ResolvedSourceTarget {
            target: SourceTarget::MacroParamReference(_),
            ..
        })
    ));
}

#[test]
fn source_graph_macro_definition_full_selection_covers_body() {
    let root_text = "`define OBJ 8\nmodule top;\nendmodule\n";
    let db = db_with_entries(&[(TOP, "rtl/top.sv", root_text)]);

    let resolved = PositionResolver::new(&db).resolve_position(
        FilePosition { file_id: TOP, offset: offset(root_text, "OBJ 8") },
        SourcePurpose::Hover,
        None,
    );
    let SourceTargetResolution::Resolved(ResolvedSourceTarget {
        model_file_id,
        entity,
        target: SourceTarget::MacroDefinition(_),
    }) = resolved
    else {
        panic!("macro definition should resolve from source graph: {resolved:?}");
    };
    assert_eq!(model_file_id, TOP);
    let source_graph = db.source_graph_preproc_model(TOP);
    let source_graph = source_graph.as_ref().as_ref().expect("source graph should build");

    let SourceRangeResult::Mapped(full_range) =
        source_graph.graph.entity_full_file_range(entity, SourcePurpose::Hover)
    else {
        panic!("macro definition full selection should map to file range");
    };

    assert_eq!(text_at_range(root_text, full_range.range), "`define OBJ 8");
}

#[test]
fn source_graph_indexes_macro_expansion_relations() {
    let root_text = "`define OBJ 8\nmodule top;\nlocalparam int W = `OBJ;\nendmodule\n";
    let db = db_with_entries(&[(TOP, "rtl/top.sv", root_text)]);

    let resolved = PositionResolver::new(&db).resolve_position(
        FilePosition { file_id: TOP, offset: offset(root_text, "OBJ;") },
        SourcePurpose::Hover,
        None,
    );
    let SourceTargetResolution::Resolved(ResolvedSourceTarget {
        model_file_id,
        entity: reference_entity,
        target: SourceTarget::MacroReference(_),
    }) = resolved
    else {
        panic!("macro reference should resolve from source graph: {resolved:?}");
    };
    assert_eq!(model_file_id, TOP);
    let source_graph = db.source_graph_preproc_model(TOP);
    let source_graph = source_graph.as_ref().as_ref().expect("source graph should build");
    let graph = &source_graph.graph;

    let call = graph
        .entity_parents(reference_entity)
        .iter()
        .find_map(|parent| match graph.entity(*parent) {
            SourceEntity::MacroCall(call) => Some(call),
            _ => None,
        })
        .expect("macro reference should be contained by a macro call");
    let expansion = graph
        .expansion_for_call(source_graph.root_context, call)
        .expect("macro call should expand");
    let emitted = graph.emitted_tokens(expansion);
    assert!(!emitted.is_empty(), "expansion should emit token entities");

    let selection =
        graph.entity_selection(emitted[0]).expect("emitted token should have selection");
    let emitted_span = graph.selection(selection).full;
    let spelled_sources = graph.spelled_sources(emitted_span);
    assert!(!spelled_sources.is_empty(), "emitted token should retain spelling provenance");
    let source_span = spelled_sources[0].0;
    assert!(
        graph
            .generated_from_spelling_source(source_span)
            .iter()
            .any(|(generated, _)| *generated == emitted_span),
        "spelling provenance should be queryable from source to generated span"
    );
    assert!(
        graph
            .generated_from_file_position(FilePosition {
                file_id: TOP,
                offset: offset(root_text, "8"),
            })
            .iter()
            .any(|(generated, _)| *generated == emitted_span),
        "spelling provenance should be queryable from source file position"
    );
}

#[test]
fn position_resolver_resolves_include_target_from_source_graph() {
    let root_text = "`include \"defs.vh\"\nmodule top;\nendmodule\n";
    let header_text = "`define HEADER_WIDTH 8\n";
    let db = db_with_files(root_text, header_text);

    let resolved = PositionResolver::new(&db).resolve_position(
        FilePosition { file_id: TOP, offset: offset(root_text, "defs.vh") },
        SourcePurpose::GotoDefinition,
        None,
    );

    assert!(matches!(
        resolved,
        SourceTargetResolution::Resolved(ResolvedSourceTarget {
            target: SourceTarget::Include(_),
            ..
        })
    ));
}

#[test]
fn hir_file_source_map_records_written_module_origin() {
    let root_text = "module top;\nendmodule\n";
    let db = db_with_entries(&[(TOP, "rtl/top.sv", root_text)]);

    let (hir_file, source_map) = db.hir_file_with_source_map(HirFileId(TOP));
    let (module_id, _) = hir_file.modules.iter().next().expect("module should lower");
    let module_src = source_map.get(module_id).expect("module source should map");
    let origin = source_map.module_origin(module_id).expect("module should have written origin");

    let source_graph = db.source_graph_preproc_model(TOP);
    let source_graph = source_graph.as_ref().as_ref().expect("source graph should build");
    assert_eq!(
        source_graph
            .graph
            .written_origin_for_file_range(FileRange { file_id: TOP, range: module_src.range() }),
        Some(origin)
    );
}

#[test]
fn preproc_macro_call_resolutions_in_range_map_formal_params() {
    let root_text = "\
`define MAKE(width, expr) logic [width-1:0] expr
module top; `MAKE(8, data_q) endmodule
";
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let start = offset(root_text, "`MAKE");
    let end = offset_after(root_text, "data_q");

    let resolutions =
        macro_call_resolutions_in_range(&db, TOP, TextRange::new(start, end)).unwrap();

    assert_eq!(resolutions.len(), 1);
    let resolution = &resolutions[0];
    assert_eq!(text_at_range(root_text, resolution.call.range), "`MAKE(8, data_q)");
    assert_eq!(
        resolution
            .definition
            .params
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|param| param.name.as_deref())
            .collect::<Vec<_>>(),
        vec!["width", "expr"]
    );
    assert_eq!(
        resolution
            .call
            .arguments
            .iter()
            .filter_map(|argument| argument.range.map(|range| text_at_range(root_text, range)))
            .collect::<Vec<_>>(),
        vec!["8", "data_q"]
    );
}

#[test]
fn macro_generated_declaration_hir_range_records_source_graph_origin() {
    let root_text = r#"`define MAKE_DECL(name) logic name;
module top;
`MAKE_DECL(generated)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let (hir_file, _) = db.hir_file_with_source_map(TOP.into());
    let (local_module_id, _) = hir_file.modules.iter().next().unwrap();
    let module_id: ModuleId = InFile::new(TOP.into(), local_module_id);
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (declaration_id, _) =
        module.declarations.iter().next().expect("generated declaration should lower to HIR");
    let declaration_src = module_src_map
        .get(declaration_id)
        .expect("generated declaration should keep a source-map range");
    let origin = module_src_map
        .declaration_origin(declaration_id)
        .expect("generated declaration should keep a SourceGraph origin");

    let source_graph = db.source_graph_preproc_model(TOP);
    let source_graph = source_graph.as_ref().as_ref().expect("source graph should build");
    let SourceOrigin::Composite { origins, preferred_span } = source_graph.graph.origin(origin)
    else {
        panic!("generated declaration should use a composite macro expansion origin");
    };
    assert_eq!(
        source_graph.graph.to_file_range(
            preferred_span.expect("composite origin should prefer the macro call span"),
            source_model::SourcePurpose::Diagnostic,
        ),
        source_model::SourceRangeResult::Mapped(FileRange {
            file_id: TOP,
            range: declaration_src.range(),
        })
    );
    assert_eq!(origins.len(), 3);
}

#[test]
fn preproc_visible_macro_names_include_predefines_without_file_mapping() {
    let root_text = r#"`define A005_LOCAL 1
module top;
localparam int W = `A005_;
endmodule
"#;
    let db = db_with_entries_and_predefines(
        &[(TOP, "rtl/top.v", root_text)],
        vec!["A005_MAGIC=42".to_owned()],
    );

    let names = visible_macro_names_at(&db, TOP, offset_after(root_text, "`A005_")).unwrap();

    assert!(names.iter().any(|name| name == "A005_LOCAL"), "{names:?}");
    assert!(names.iter().any(|name| name == "A005_MAGIC"), "{names:?}");
}

#[test]
fn preproc_single_offset_contexts_exclude_unrelated_profile_models() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let unrelated_header_text = "`define UNUSED_WIDTH 16\n";
    let db = db_with_nested_files(root_text, header_text, unrelated_header_text);

    let contexts = source_preproc_single_query_contexts(&db, HEADER);

    assert!(contexts.model_file_ids.contains(&TOP), "{contexts:?}");
    assert!(!contexts.model_file_ids.contains(&HEADER), "{contexts:?}");
    assert!(
        !contexts.model_file_ids.contains(&LEAF),
        "single-offset query contexts should not include unrelated profile model: {contexts:?}"
    );
}

#[test]
fn preproc_header_without_including_context_uses_standalone_model() {
    let root_text = "module top; endmodule\n";
    let header_text = "`define WIDTH 8\n";
    let db = db_with_files(root_text, header_text);

    let contexts = source_preproc_single_query_contexts(&db, HEADER);

    assert!(contexts.model_file_ids.contains(&HEADER), "{contexts:?}");
    assert!(!contexts.model_file_ids.contains(&TOP), "{contexts:?}");
}

#[test]
fn preproc_partial_context_index_is_structured_unavailable() {
    let contexts = SourcePreprocQueryContexts {
        model_file_ids: Vec::new(),
        status: SourcePreprocContextStatus::Partial { skipped_models: 2 },
    };

    let error = finish_empty_single_query(&contexts, None).unwrap_err();

    assert!(matches!(
        error,
        PreprocError::Unavailable {
            reason: PreprocUnavailable::PartialPreprocContextIndex { skipped_models: 2 }
        }
    ));
}

#[test]
fn preproc_partial_context_index_marks_nonempty_results_partial() {
    let contexts = SourcePreprocQueryContexts {
        model_file_ids: vec![TOP],
        status: SourcePreprocContextStatus::Partial { skipped_models: 2 },
    };

    assert_eq!(
        context_query_capability(&contexts, PreprocAvailability::Complete),
        PreprocAvailability::Partial
    );
    assert_eq!(
        context_query_capability(&contexts, PreprocAvailability::Partial),
        PreprocAvailability::Partial
    );
}

#[test]
fn preproc_visible_macro_names_follow_define_undef_boundaries() {
    let root_text = r#"`define A005_LOCAL 1
`undef A005_LOCAL
`define A005_NEXT 2
module top;
localparam int W = `A005_;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let names_after_define =
        visible_macro_names_at(&db, TOP, offset_after(root_text, "`define A005_LOCAL 1\n"))
            .unwrap();
    let names_after_undef =
        visible_macro_names_at(&db, TOP, offset_after(root_text, "`undef A005_LOCAL\n")).unwrap();
    let names_after_next =
        visible_macro_names_at(&db, TOP, offset_after(root_text, "`define A005_NEXT 2\n")).unwrap();

    assert!(names_after_define.iter().any(|name| name == "A005_LOCAL"));
    assert!(!names_after_undef.iter().any(|name| name == "A005_LOCAL"));
    assert!(names_after_next.iter().any(|name| name == "A005_NEXT"));
}

#[test]
fn preproc_inactive_branch_uses_header_define() {
    let root_text = r#"`include "defs.vh"
`ifndef HEADER_FLAG
wire disabled_by_header;
`endif
wire active;
"#;
    let header_text = "`define HEADER_FLAG\n";
    let db = db_with_files(root_text, header_text);

    let branches = inactive_branches(&db, TOP).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].file_id, TOP);
    assert!(text_at_range(root_text, branches[0].range).contains("disabled_by_header"));
}

#[test]
fn source_graph_included_define_references_include_root_conditionals() {
    let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
localparam int ENABLED = `HEADER_FLAG;
`endif
"#;
    let header_text = "`define HEADER_FLAG 1\n";
    let db = db_with_files(root_text, header_text);
    let definition_range = TextRange::new(
        offset_after(header_text, "`define "),
        offset_after(header_text, "`define HEADER_FLAG"),
    );

    let refs = source_graph_macro_references_for_definition(&db, TOP, HEADER, definition_range);

    assert!(refs.iter().any(|(reference, _)| {
        reference.file_id == TOP && text_at_range(root_text, reference.range) == "HEADER_FLAG"
    }));
    assert!(refs.iter().any(|(reference, reason)| {
        reference.file_id == TOP
            && *reason == ResolutionReason::VisibleDefinition
            && text_at_range(root_text, reference.range) == "HEADER_FLAG"
    }));
}

#[test]
fn source_graph_ifndef_guard_reference_resolves_to_following_define() {
    let root_text = "`include \"defs.vh\"\n";
    let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
    let db = db_with_files(root_text, header_text);
    let definition_range = TextRange::new(
        offset_after(header_text, "`define "),
        offset_after(header_text, "`define HEADER_FLAG"),
    );

    let refs = source_graph_macro_references_for_definition(&db, TOP, HEADER, definition_range);
    assert!(refs.iter().any(|(reference, _)| {
        reference.file_id == HEADER
            && reference.range.start() == offset(header_text, "HEADER_FLAG")
            && text_at_range(header_text, reference.range) == "HEADER_FLAG"
    }));
}

#[test]
fn source_graph_project_header_guard_reference_is_indexed_without_include() {
    let root_text = "module top; endmodule\n";
    let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
    let db = db_with_files(root_text, header_text);
    let definition_range = TextRange::new(
        offset_after(header_text, "`define "),
        offset_after(header_text, "`define HEADER_FLAG"),
    );

    let refs = source_graph_macro_references_for_definition(&db, HEADER, HEADER, definition_range);
    assert!(refs.iter().any(|(reference, _)| {
        reference.file_id == HEADER && text_at_range(header_text, reference.range) == "HEADER_FLAG"
    }));
}
