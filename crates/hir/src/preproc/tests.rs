use std::fmt;

use rustc_hash::FxHashSet;
use source_model::{
    FilePosition, FileRange, ResolvedSourceTarget, SourceOrigin, SourcePurpose, SourceRangeResult,
    SourceTarget, SourceTargetResolution,
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
            FileLoader, PreprocExpansionSourceBuffer, PreprocVirtualOrigin, SourceDb,
            SourceDbStorage, SourceFileKind, SourceRootDb, SourceRootDbStorage,
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

fn offset_after_n(text: &str, needle: &str, occurrence: usize) -> TextSize {
    let mut cursor = 0;
    for index in 0..=occurrence {
        let relative = text[cursor..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing occurrence {occurrence} of {needle:?} in fixture"));
        let absolute = cursor + relative;
        if index == occurrence {
            return TextSize::from(u32::try_from(absolute + needle.len()).unwrap());
        }
        cursor = absolute + needle.len();
    }
    unreachable!()
}

fn text_at_range(text: &str, range: TextRange) -> &str {
    &text[usize::from(range.start())..usize::from(range.end())]
}

fn assert_expansion_is_display_only_source_buffer(
    mapped: &MappedSourcePreprocModel,
    expansion: &MacroExpansion,
) {
    let expansion_id = SourceMacroExpansionId::new(expansion.id.raw());
    let entry =
        mapped.source_map.expansion(expansion_id).expect("expansion should have a display entry");
    assert!(matches!(&entry.source_buffer, PreprocExpansionSourceBuffer::DisplayOnly { .. }));
    assert!(matches!(
        mapped.source_map.emitted_source_buffer_range(expansion_id, expansion.emitted_token_range),
        Err(PreprocSourceMapError::DisplayOnlyVirtualSource { .. })
    ));
}

#[test]
fn preproc_include_usage_resolves_to_header_define() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let db = db_with_files(root_text, header_text);

    let resolution =
        macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH")).unwrap().unwrap();
    assert_eq!(resolution.usage.file_id, TOP);
    assert_eq!(resolution.definition.file_id, HEADER);
    assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
    assert_eq!(text_at_range(header_text, resolution.definition.name_range), "HEADER_WIDTH");

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
        entity,
        target: SourceTarget::MacroDefinition(_),
    }) = resolved
    else {
        panic!("macro definition should resolve from source graph: {resolved:?}");
    };
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
fn preproc_macro_expansion_queries_map_call_ranges() {
    let root_text = r#"`define OBJ 8
`define LEAF 3
`define WRAP `LEAF
module top;
localparam int A = `OBJ;
localparam int B = `WRAP;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let immediate =
        immediate_macro_expansion_at(&db, TOP, offset(root_text, "`OBJ")).unwrap().unwrap();
    let MacroExpansionQuery::Available(immediate) = immediate else {
        panic!("object-like macro expansion should be available");
    };
    assert_eq!(immediate.call.file_id, TOP);
    assert_eq!(text_at_range(root_text, immediate.call.range), "`OBJ");
    assert_eq!(immediate.emitted_token_range.len, 1);
    assert!(matches!(immediate.capability, PreprocAvailability::Complete));

    let recursive =
        recursive_macro_expansion_at(&db, TOP, offset(root_text, "`WRAP")).unwrap().unwrap();
    assert_eq!(recursive.root_call.file_id, TOP);
    assert_eq!(text_at_range(root_text, recursive.root_call.range), "`WRAP");
    assert!(recursive.unavailable.is_empty());
    assert_eq!(recursive.expansions.len(), 2);
    let wrap_expansion = recursive
        .expansions
        .iter()
        .find(|expansion| expansion.definition.name().as_str() == "WRAP")
        .expect("outer expansion should be mapped");
    let leaf_expansion = recursive
        .expansions
        .iter()
        .find(|expansion| expansion.definition.name().as_str() == "LEAF")
        .expect("nested expansion should be mapped");
    assert_eq!(text_at_range(root_text, wrap_expansion.call.range), "`WRAP");
    assert_eq!(text_at_range(root_text, leaf_expansion.call.range), "`LEAF");
    assert_eq!(wrap_expansion.child_calls, vec![leaf_expansion.call.id]);
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
fn preproc_builtin_intrinsic_expansion_uses_structured_provenance() {
    let root_text = r#"module m;
localparam int L = `__LINE__;
localparam string F = `__FILE__;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let line_offset = offset(root_text, "`__LINE__");
    let file_offset = offset(root_text, "`__FILE__");
    for (offset, expected_name) in [(line_offset, "__LINE__"), (file_offset, "__FILE__")] {
        let immediate =
            immediate_macro_expansion_at(&db, TOP, offset).unwrap().expect("builtin call expected");
        let MacroExpansionQuery::Available(immediate) = immediate else {
            panic!("builtin macro expansion should be available");
        };
        assert_eq!(immediate.definition.name().as_str(), expected_name);
        assert!(matches!(
            immediate.definition,
            MacroExpansionDefinition::Builtin { name, .. } if name.as_str() == expected_name
        ));

        let recursive =
            recursive_macro_expansion_at(&db, TOP, offset).unwrap().expect("recursive expected");
        assert!(recursive.unavailable.is_empty());
        assert!(recursive.expansions.iter().any(|expansion| {
            matches!(
                &expansion.definition,
                MacroExpansionDefinition::Builtin { name, .. } if name.as_str() == expected_name
            )
        }));

        let provenance =
            macro_expansion_provenance_at(&db, TOP, offset).unwrap().expect("provenance expected");
        assert!(provenance.tokens.iter().any(|token| {
            matches!(
                &token.provenance,
                TokenProvenance::Builtin { name, call }
                    if name.as_str() == expected_name && call.range == provenance.expansion.call.range
            )
        }));

        let diagnostic = diagnostic_provenance_for_range(&db, TOP, provenance.expansion.call.range)
            .unwrap()
            .expect("diagnostic provenance expected");
        assert!(matches!(
            diagnostic,
            DiagnosticProvenance::Builtin { name, call }
                if name.as_str() == expected_name && call.range == provenance.expansion.call.range
        ));
    }
}

#[test]
fn preproc_zero_token_macro_expansion_is_available() {
    let root_text = r#"`define EMPTY
`define DROP(x)
module top;
`EMPTY
`DROP(foo)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    for name in ["`EMPTY", "`DROP"] {
        let immediate =
            immediate_macro_expansion_at(&db, TOP, offset(root_text, name)).unwrap().unwrap();
        let MacroExpansionQuery::Available(immediate) = immediate else {
            panic!("{name} expansion should be available");
        };
        assert_eq!(immediate.emitted_token_range.len, 0);

        let provenance =
            macro_expansion_provenance_at(&db, TOP, offset(root_text, name)).unwrap().unwrap();
        assert!(provenance.tokens.is_empty());
        assert_eq!(provenance.expansion.emitted_token_range.len, 0);

        let mapped = db.source_preproc_model(TOP);
        let mapped = mapped.as_ref().as_ref().unwrap();
        let display_text = mapped
            .source_map
            .expansion_display_text(SourceMacroExpansionId::new(provenance.expansion.id.raw()))
            .unwrap();
        assert_eq!(display_text, "");
        assert_eq!(provenance.expansion.display_range, TextRange::empty(TextSize::from(0)));
    }
}

#[test]
fn preproc_macro_expansion_exposes_display_virtual_source_and_token_provenance() {
    let root_text = r#"`define MAKE_DECL(name) logic name;
module top;
`MAKE_DECL(generated)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let provenance =
        macro_expansion_provenance_at(&db, TOP, offset(root_text, "`MAKE_DECL")).unwrap().unwrap();
    let MappedPreprocSource::VirtualDisplay { path, origin } = &provenance.expansion.display_source
    else {
        panic!("macro expansion should expose a display-only virtual expansion source");
    };
    assert_eq!(
        path,
        &VfsPath::new_virtual_path("/__vide/preproc/profile-0/expansion/0.sv".to_owned())
    );
    assert_eq!(
        origin,
        &PreprocVirtualOrigin::Expansion { expansion: SourceMacroExpansionId::new(0) }
    );

    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().unwrap();
    let expansion_display =
        mapped.source_map.expansion_display_text(SourceMacroExpansionId::new(0)).unwrap();
    assert_eq!(expansion_display, "\nlogic generated;");
    assert_eq!(provenance.expansion.display_text, expansion_display);
    assert_eq!(provenance.expansion.display_range, TextRange::new(1.into(), 17.into()));

    let logic = provenance
        .tokens
        .iter()
        .find(|token| token.text.as_str() == "logic")
        .expect("macro body token should be present");
    let TokenProvenance::MacroBody { identity: logic_identity, source, range, .. } =
        &logic.provenance
    else {
        panic!("logic should come from the macro body: {logic:?}");
    };
    assert_eq!(source.file_id(), Some(TOP));
    assert_eq!(text_at_range(root_text, *range), "logic");
    assert_eq!(logic.display_range, TextRange::new(1.into(), 6.into()));
    assert_eq!(logic_identity.body_token_index, 0);

    let generated = provenance
        .tokens
        .iter()
        .find(|token| token.text.as_str() == "generated")
        .expect("macro argument token should be present");
    let TokenProvenance::MacroArgument {
        identity: generated_identity,
        source,
        range,
        argument_index,
        ..
    } = &generated.provenance
    else {
        panic!("generated should come from the macro argument: {generated:?}");
    };
    assert_eq!(*argument_index, 0);
    assert_eq!(source.file_id(), Some(TOP));
    assert_eq!(text_at_range(root_text, *range), "generated");
    assert_eq!(generated.display_range, TextRange::new(7.into(), 16.into()));
    assert_eq!(generated_identity.call, logic_identity.call);
    assert_eq!(generated_identity.definition, logic_identity.definition);
    assert_eq!(generated_identity.parent_expansion, Some(logic_identity.expansion));
    assert_eq!(generated_identity.body_token_index, 1);
    assert_eq!(generated_identity.argument_index, 0);
    assert_eq!(generated_identity.argument_token_index, 0);
}

#[test]
fn preproc_macro_expansion_display_keeps_emitted_token_trivia() {
    let root_text = r#"`define BLOCK(name) \
  always_ff @(posedge clk) begin \
    name <= 1; \
  end
module top;
  `BLOCK(q)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let provenance =
        macro_expansion_provenance_at(&db, TOP, offset(root_text, "`BLOCK")).unwrap().unwrap();
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().unwrap();
    let display_text = mapped
        .source_map
        .expansion_display_text(SourceMacroExpansionId::new(provenance.expansion.id.raw()))
        .unwrap();

    assert_eq!(provenance.expansion.display_text, display_text);
    assert!(
        display_text.contains("\n  always_ff")
            && display_text.contains("\n    q <= 1;")
            && display_text.contains("\n  end"),
        "expansion display text should preserve emitted token trivia: {display_text:?}"
    );
}

#[test]
fn preproc_maps_nested_actual_argument_macro_usage_without_dropping_expansion() {
    let root_text = r#"`define PAYL payload_i
`define NEXT(x) ((x) + 12'd1)
module top(input logic [3:0] payload_i, output logic [3:0] y);
assign y = `NEXT(`PAYL);
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let payl = macro_reference_definitions_at(&db, TOP, offset_after(root_text, "`NEXT("))
        .unwrap()
        .expect("nested actual-argument macro reference should be mapped");
    assert_eq!(text_at_range(root_text, payl.range), "`PAYL");
    assert!(
        payl.definitions
            .iter()
            .any(|definition| { definition.file_id == TOP && definition.name.as_str() == "PAYL" })
    );

    let provenance =
        macro_expansion_provenance_at(&db, TOP, offset(root_text, "`NEXT")).unwrap().unwrap();
    let argument = provenance
        .expansion
        .call
        .arguments
        .iter()
        .find(|argument| argument.argument_index == 0)
        .expect("NEXT call should expose its written actual argument");
    assert_eq!(argument.source.as_ref().and_then(MappedPreprocSource::file_id), Some(TOP));
    assert_eq!(text_at_range(root_text, argument.range.unwrap()), "`PAYL");
    assert_eq!(
        argument.tokens.iter().map(|token| token.raw.as_str()).collect::<Vec<_>>(),
        vec!["`PAYL"]
    );

    let payload = provenance
        .tokens
        .iter()
        .find(|token| token.text.as_str() == "payload_i")
        .expect("expanded payload token should stay in NEXT expansion provenance");
    let TokenProvenance::MacroBody { call, source, range, .. } = &payload.provenance else {
        panic!("nested PAYL expansion should keep direct macro body provenance: {payload:?}");
    };
    assert_eq!(source.file_id(), Some(TOP));
    assert_eq!(text_at_range(root_text, *range), "payload_i");
    assert_eq!(text_at_range(root_text, call.range), "`PAYL");

    let payl_offset = offset(root_text, "`PAYL");
    let queries = macro_expansion_queries_at(&db, TOP, payl_offset).unwrap();
    assert!(queries.iter().any(|query| matches!(
        query,
        MacroExpansionQuery::Available(expansion)
            if expansion.definition.name().as_str() == "NEXT"
    )));
    assert!(queries.iter().any(|query| matches!(
        query,
        MacroExpansionQuery::Available(expansion)
            if expansion.definition.name().as_str() == "PAYL"
    )));
    assert!(!queries.iter().any(|query| matches!(query, MacroExpansionQuery::Unavailable(_))));
    assert!(matches!(
        immediate_macro_expansion_at(&db, TOP, payl_offset),
        Ok(Some(MacroExpansionQuery::Ambiguous(expansions)))
            if expansions.len() == 2
                && expansions.iter().any(|expansion| expansion.definition.name().as_str() == "NEXT")
                && expansions.iter().any(|expansion| expansion.definition.name().as_str() == "PAYL")
    ));
    assert!(matches!(
        macro_expansion_provenance_at(&db, TOP, payl_offset),
        Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts: 2 }
        })
    ));
}

#[test]
fn preproc_numeric_literal_expansion_display_is_not_source_buffer() {
    let root_text = r#"`define ONE 12'd1
module top;
localparam int W = `ONE;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let provenance =
        macro_expansion_provenance_at(&db, TOP, offset(root_text, "`ONE")).unwrap().unwrap();
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().unwrap();
    assert_expansion_is_display_only_source_buffer(mapped, &provenance.expansion);

    let display_text = mapped
        .source_map
        .expansion_display_text(SourceMacroExpansionId::new(provenance.expansion.id.raw()))
        .unwrap();
    assert!(display_text.contains("12"));
    assert!(display_text.contains("'d"));
    assert!(display_text.contains("1"));
}

#[test]
fn preproc_escaped_identifier_expansion_display_is_not_source_buffer() {
    let root_text = concat!(
        "`define ESCAPED \\escaped.name \n",
        "module top;\n",
        "wire `ESCAPED;\n",
        "endmodule\n",
    );
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let provenance =
        macro_expansion_provenance_at(&db, TOP, offset(root_text, "`ESCAPED")).unwrap().unwrap();
    let mapped = db.source_preproc_model(TOP);
    let mapped = mapped.as_ref().as_ref().unwrap();
    assert_expansion_is_display_only_source_buffer(mapped, &provenance.expansion);

    let display_text = mapped
        .source_map
        .expansion_display_text(SourceMacroExpansionId::new(provenance.expansion.id.raw()))
        .unwrap();
    assert!(display_text.contains("\\escaped.name"));
}

#[test]
fn macro_generated_declaration_hir_range_resolves_to_expanded_token_provenance() {
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

    let provenance =
        macro_expansion_provenance_for_range(&db, TOP, declaration_src.range()).unwrap().unwrap();

    assert_eq!(provenance.expansion.emitted_token_range.len, 3);
    assert!(
        provenance
            .tokens
            .iter()
            .any(|token| matches!(token.provenance, TokenProvenance::MacroBody { .. }))
    );
    assert!(
        provenance
            .tokens
            .iter()
            .any(|token| matches!(token.provenance, TokenProvenance::MacroArgument { .. }))
    );
}

#[test]
fn diagnostic_provenance_for_range_spanning_two_macro_calls_is_ambiguous() {
    let root_text = r#"`define A 1
`define B 2
module top;
localparam int W = `A + `B;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let range = TextRange::new(offset(root_text, "`A"), offset_after(root_text, "`B"));

    let provenance = diagnostic_provenance_for_range(&db, TOP, range).unwrap().unwrap();

    assert!(matches!(
        provenance,
        DiagnosticProvenance::Unavailable(PreprocUnavailable::AmbiguousDiagnosticProvenance {
            targets: 2
        })
    ));
    let expansion_error = macro_expansion_provenances_for_range(&db, TOP, range).unwrap_err();
    assert!(matches!(
        expansion_error,
        PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts: 2 }
        }
    ));
}

#[test]
fn diagnostic_provenance_for_adjacent_macro_calls_only_hits_intersecting_call() {
    let root_text = r#"`define ID(x) x
module top;
localparam int W = `ID(1)`ID(2);
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let two_range = TextRange::new(offset(root_text, "`ID(2)"), offset_after(root_text, "`ID(2)"));

    let provenance = diagnostic_provenance_for_range(&db, TOP, two_range).unwrap().unwrap();

    let DiagnosticProvenance::MacroArgument { call, argument_index, source, range } = provenance
    else {
        panic!("adjacent single-call range should resolve precisely: {provenance:?}");
    };
    assert_eq!(text_at_range(root_text, call.range), "`ID(2)");
    assert_eq!(argument_index, 0);
    assert_eq!(source.file_id(), Some(TOP));
    assert_eq!(text_at_range(root_text, range), "2");
}

#[test]
fn diagnostic_provenance_for_nested_macro_call_range_is_precise() {
    let root_text = r#"`define LEAF 3
`define WRAP `LEAF
module top;
localparam int W = `WRAP;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let leaf_range = TextRange::new(offset(root_text, "`LEAF"), offset_after(root_text, "`LEAF"));

    let provenance = diagnostic_provenance_for_range(&db, TOP, leaf_range).unwrap().unwrap();

    let DiagnosticProvenance::MacroBody { call, source, range, .. } = provenance else {
        panic!("nested macro call range should resolve precisely");
    };
    assert_eq!(text_at_range(root_text, call.range), "`LEAF");
    assert_eq!(source.file_id(), Some(TOP));
    assert_eq!(text_at_range(root_text, range), "3");
}

#[test]
fn diagnostic_provenance_returns_unavailable_for_unsupported_expansion_mapping() {
    let root_text = r#"`define JOIN(a,b) a``b
`define STR(x) `"x`"
module top;
wire `JOIN(foo,bar);
string s = `STR(foo);
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let call_range =
        TextRange::new(offset(root_text, "`JOIN"), offset_after(root_text, "`JOIN(foo,bar)"));

    let provenance = diagnostic_provenance_for_range(&db, TOP, call_range).unwrap().unwrap();
    assert!(
        matches!(provenance, DiagnosticProvenance::Unavailable(_)),
        "token paste diagnostic provenance should be unavailable, got {provenance:?}"
    );

    let stringification_range =
        TextRange::new(offset(root_text, "`STR"), offset_after(root_text, "`STR(foo)"));
    let provenance =
        diagnostic_provenance_for_range(&db, TOP, stringification_range).unwrap().unwrap();
    assert!(
        matches!(provenance, DiagnosticProvenance::Unavailable(_)),
        "stringification diagnostic provenance should be unavailable, got {provenance:?}"
    );
}

#[test]
fn diagnostic_provenance_for_unbacked_predefine_expansion_is_structured_unavailable() {
    let root_text = r#"module top;
`MAKE_CHILD
endmodule
"#;
    let db = db_with_entries_and_predefines(
        &[(TOP, "rtl/top.v", root_text)],
        vec!["MAKE_CHILD=child u();".to_owned()],
    );
    let (hir_file, _) = db.hir_file_with_source_map(TOP.into());
    let (local_module_id, _) = hir_file.modules.iter().next().unwrap();
    let module_id: ModuleId = InFile::new(TOP.into(), local_module_id);
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (instantiation_id, _) = module
        .instantiations
        .iter()
        .next()
        .expect("predefine expansion should lower to a module instantiation");
    let instantiation_src = module_src_map
        .get(instantiation_id)
        .expect("generated instantiation should keep a source-map range");

    let provenance =
        diagnostic_provenance_for_range(&db, TOP, instantiation_src.range()).unwrap().unwrap();

    assert!(
        matches!(provenance, DiagnosticProvenance::Unavailable(_)),
        "unbacked predefine diagnostic provenance should be unavailable, got {provenance:?}"
    );
}

#[test]
fn preproc_nested_include_chain_maps_to_file_ids() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `LEAF_WIDTH;
endmodule
"#;
    let header_text = "`include \"leaf.vh\"\n";
    let leaf_text = "`define LEAF_WIDTH 4\n";
    let db = db_with_nested_files(root_text, header_text, leaf_text);

    let resolution =
        macro_usage_resolution_at(&db, TOP, offset(root_text, "LEAF_WIDTH")).unwrap().unwrap();

    assert_eq!(resolution.definition.file_id, LEAF);
    assert_eq!(resolution.definition_provenance.file_id, LEAF);
    assert_eq!(resolution.include_chain.len(), 2);
    assert_eq!(resolution.include_chain[0].include_file_id, TOP);
    assert_eq!(resolution.include_chain[0].included_file_id, HEADER);
    assert!(
        text_at_range(root_text, resolution.include_chain[0].include_range).contains("defs.vh")
    );
    assert_eq!(resolution.include_chain[1].include_file_id, HEADER);
    assert_eq!(resolution.include_chain[1].included_file_id, LEAF);
    assert!(
        text_at_range(header_text, resolution.include_chain[1].include_range).contains("leaf.vh")
    );
}

#[test]
fn preproc_unsaved_include_buffer_updates_query_result() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
    let mut db = db_with_files(root_text, "`define OTHER_WIDTH 8\n");

    assert!(
        macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH")).unwrap().is_none()
    );

    db.set_file_text_with_durability(
        HEADER,
        Arc::from("`define HEADER_WIDTH 16\n"),
        Durability::LOW,
    );

    let resolution =
        macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH")).unwrap().unwrap();
    assert_eq!(resolution.definition.file_id, HEADER);
    assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
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
fn preproc_include_only_sv_query_uses_all_including_roots() {
    let top_a_text = r#"`define WIDTH 8
`include "shared.sv"
"#;
    let shared_text = "localparam int W = `WIDTH;\n";
    let top_b_text = r#"`define WIDTH 16
`include "shared.sv"
"#;
    let db = db_with_entries(&[
        (TOP, "rtl/top_a.sv", top_a_text),
        (HEADER, "include/shared.sv", shared_text),
        (LEAF, "rtl/top_b.sv", top_b_text),
    ]);

    let plan = db.compilation_plan_for_profile(Some(PROFILE));
    assert!(plan.include_only.contains(&HEADER), "{plan:?}");
    assert!(plan.roots.contains(&TOP), "{plan:?}");
    assert!(plan.roots.contains(&LEAF), "{plan:?}");
    assert!(!plan.roots.contains(&HEADER), "{plan:?}");

    let contexts = source_preproc_single_query_contexts(&db, HEADER);
    assert!(contexts.model_file_ids.contains(&TOP), "{contexts:?}");
    assert!(contexts.model_file_ids.contains(&LEAF), "{contexts:?}");
    assert!(!contexts.model_file_ids.contains(&HEADER), "{contexts:?}");

    let definitions =
        macro_reference_definitions_at(&db, HEADER, offset(shared_text, "WIDTH")).unwrap().unwrap();

    assert_eq!(definitions.definitions.len(), 2);
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP && text_at_range(top_a_text, definition.name_range) == "WIDTH"
    }));
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == LEAF && text_at_range(top_b_text, definition.name_range) == "WIDTH"
    }));
}

#[test]
fn preproc_header_query_uses_including_context_over_standalone_model() {
    let root_text = r#"`define FEATURE 1
`include "defs.vh"
"#;
    let header_text = r#"`ifdef FEATURE
`define WIDTH 8
`endif
localparam int W = `WIDTH;
"#;
    let db = db_with_files(root_text, header_text);

    let reference = macro_reference_at(&db, HEADER, offset(header_text, "WIDTH;"))
        .unwrap()
        .expect("included context should resolve the header reference without ambiguity");
    assert_eq!(text_at_range(header_text, reference.range), "`WIDTH");
    assert!(matches!(reference.resolution, MacroResolution::Resolved { .. }));

    let resolution = macro_reference_resolution_at(&db, HEADER, offset(header_text, "WIDTH;"))
        .unwrap()
        .expect("header macro reference should resolve through the including root");
    assert_eq!(resolution.definition.file_id, HEADER);
    assert_eq!(text_at_range(header_text, resolution.definition.name_range), "WIDTH");
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
fn preproc_manifest_predefine_definition_uses_manifest_provenance() {
    let root_text = r#"`ifdef Z_FROM_MANIFEST
module top;
localparam int W = `Z_FROM_MANIFEST;
endmodule
`endif
"#;
    let manifest_text = "defines = [\"A_OTHER=2\", \"Z_FROM_MANIFEST=1\"]\n";
    let manifest_range = TextRange::new(
        offset(manifest_text, "\"Z_FROM_MANIFEST=1\""),
        offset_after(manifest_text, "\"Z_FROM_MANIFEST=1\""),
    );
    let other_range = TextRange::new(
        offset(manifest_text, "\"A_OTHER=2\""),
        offset_after(manifest_text, "\"A_OTHER=2\""),
    );
    let predefine = Predefine::with_source(
        "Z_FROM_MANIFEST=1",
        PredefineSource { path: abs_path("vide.toml"), range: manifest_range },
    );
    let other_predefine = Predefine::with_source(
        "A_OTHER=2",
        PredefineSource { path: abs_path("vide.toml"), range: other_range },
    );
    let db = db_with_entries_and_predefine_entries(
        &[(TOP, "rtl/top.v", root_text), (MANIFEST, "vide.toml", manifest_text)],
        vec![other_predefine, predefine],
    );

    let resolution =
        macro_reference_definitions_at(&db, TOP, offset(root_text, "Z_FROM_MANIFEST;"))
            .unwrap()
            .unwrap();
    assert!(
        resolution.definitions.iter().any(|definition| {
            definition.file_id == MANIFEST && definition.name_range == manifest_range
        }),
        "predefine reference should target the manifest source range: {resolution:?}"
    );

    let definition = macro_definition_at(&db, MANIFEST, manifest_range.start()).unwrap().unwrap();
    assert_eq!(definition.file_id, MANIFEST);
    assert_eq!(definition.name.as_str(), "Z_FROM_MANIFEST");
    assert_eq!(definition.name_range, manifest_range);
    assert_eq!(text_at_range(manifest_text, definition.name_range), "\"Z_FROM_MANIFEST=1\"");

    let references = macro_references(&db, MANIFEST, &definition).unwrap();
    assert!(
        references.references.iter().any(|reference| {
            reference.file_id == TOP
                && text_at_range(root_text, reference.range) == "Z_FROM_MANIFEST"
        }),
        "manifest predefine definition should find source references: {references:?}"
    );
}

#[test]
fn preproc_manifest_escaped_predefine_definition_uses_manifest_provenance() {
    let root_text = r#"`ifdef MSG
module top;
localparam string S = `MSG;
endmodule
`endif
"#;
    let manifest_text = r#"defines = ["MSG=\"hello\""]
"#;
    let raw_define = r#""MSG=\"hello\"""#;
    let manifest_range =
        TextRange::new(offset(manifest_text, raw_define), offset_after(manifest_text, raw_define));
    let predefine = Predefine::with_source(
        r#"MSG="hello""#,
        PredefineSource { path: abs_path("vide.toml"), range: manifest_range },
    );
    let db = db_with_entries_and_predefine_entries(
        &[(TOP, "rtl/top.v", root_text), (MANIFEST, "vide.toml", manifest_text)],
        vec![predefine],
    );

    let definition = macro_definition_at(&db, MANIFEST, manifest_range.start()).unwrap().unwrap();
    assert_eq!(definition.file_id, MANIFEST);
    assert_eq!(definition.name.as_str(), "MSG");
    assert_eq!(definition.name_range, manifest_range);
    assert_eq!(text_at_range(manifest_text, definition.name_range), raw_define);

    let references = macro_references(&db, MANIFEST, &definition).unwrap();
    assert!(
        references.references.iter().any(|reference| reference.file_id == TOP
            && text_at_range(root_text, reference.range) == "MSG"),
        "escaped manifest predefine should still find source references: {references:?}"
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
fn preproc_included_define_references_include_root_conditionals() {
    let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
localparam int ENABLED = `HEADER_FLAG;
`endif
"#;
    let header_text = "`define HEADER_FLAG 1\n";
    let db = db_with_files(root_text, header_text);
    let definition =
        macro_definition_at(&db, HEADER, offset_after(header_text, "`define ")).unwrap().unwrap();

    assert_eq!(definition.source.file_id(), Some(HEADER));
    assert!(matches!(definition.capability, PreprocAvailability::Complete));

    let refs = macro_references(&db, HEADER, &definition).unwrap().references;

    assert!(refs.iter().any(|reference| {
        reference.file_id == TOP && text_at_range(root_text, reference.range) == "HEADER_FLAG"
    }));
    assert!(refs.iter().any(|reference| {
        reference.file_id == TOP
            && matches!(
                reference.resolution,
                MacroResolution::Resolved { reason: MacroResolutionReason::VisibleDefinition, .. }
            )
            && text_at_range(root_text, reference.range) == "HEADER_FLAG"
    }));

    let definitions =
        macro_reference_definitions_at(&db, TOP, offset_after(root_text, "ENABLED = `"))
            .unwrap()
            .unwrap();
    assert_eq!(text_at_range(root_text, definitions.range), "`HEADER_FLAG");
    assert!(macro_reference_definitions_at(&db, TOP, definitions.range.end()).unwrap().is_none());
    assert!(macro_usage_resolution_at(&db, TOP, definitions.range.end()).unwrap().is_none());
    assert!(matches!(definitions.capability, PreprocAvailability::Complete));
    assert!(definitions.definitions.iter().any(|indexed| {
        indexed.file_id == HEADER
            && indexed.name_range == definition.name_range
            && indexed.name == definition.name
    }));
}

#[test]
fn preproc_header_ifdef_reference_uses_including_root_context() {
    let root_text = r#"`include "defs.vh"
`include "leaf.vh"
"#;
    let header_text = "`define FEATURE_B 1\n";
    let leaf_text = r#"`ifdef FEATURE_B
wire enabled;
`endif
"#;
    let db = db_with_nested_files(root_text, header_text, leaf_text);

    let definitions =
        macro_reference_definitions_at(&db, LEAF, offset(leaf_text, "FEATURE_B")).unwrap().unwrap();

    assert_eq!(text_at_range(leaf_text, definitions.range), "FEATURE_B");
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == HEADER
            && text_at_range(header_text, definition.name_range) == "FEATURE_B"
    }));
}

#[test]
fn preproc_header_macro_body_references_use_expansion_context() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `DEMO_WIDTH;
localparam int N = `DEMO_NEXT(1);
localparam int R = `DEMO_RESET;
endmodule
"#;
    let header_text = r#"`ifndef SHARED_DEFS_SVH
`define SHARED_DEFS_SVH
`include "leaf.vh"
`define DEMO_WIDTH `MATH_WIDTH
`define DEMO_RESET {`DEMO_WIDTH{1'b0}}
`define DEMO_NEXT(value) ((value) + `MATH_ONE)
`endif
"#;
    let leaf_text = r#"`define MATH_WIDTH 12
`define MATH_ONE 12'd1
"#;
    let db = db_with_nested_files(root_text, header_text, leaf_text);

    let math_width = macro_reference_definitions_at(&db, HEADER, offset(header_text, "MATH_WIDTH"))
        .unwrap()
        .unwrap();
    assert!(math_width.definitions.iter().any(|definition| {
        definition.file_id == LEAF
            && text_at_range(leaf_text, definition.name_range) == "MATH_WIDTH"
    }));

    let math_one = macro_reference_definitions_at(&db, HEADER, offset(header_text, "MATH_ONE"))
        .unwrap()
        .unwrap();
    assert!(math_one.definitions.iter().any(|definition| {
        definition.file_id == LEAF && text_at_range(leaf_text, definition.name_range) == "MATH_ONE"
    }));

    let demo_width = macro_reference_definitions_at(
        &db,
        HEADER,
        offset_after(header_text, "`define DEMO_RESET {`"),
    )
    .unwrap()
    .unwrap();
    assert!(demo_width.definitions.iter().any(|definition| {
        definition.file_id == HEADER
            && text_at_range(header_text, definition.name_range) == "DEMO_WIDTH"
    }));
}

#[test]
fn preproc_macro_param_references_resolve_to_formals() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `SHIFT(4, 1);
endmodule
"#;
    let header_text = "`define SHIFT(value, amount) ((value) << amount)\n";
    let db = db_with_files(root_text, header_text);

    let value_definition =
        macro_param_definition_at(&db, HEADER, offset_after(header_text, "SHIFT("))
            .unwrap()
            .unwrap();
    assert_eq!(value_definition.name.as_str(), "value");
    assert_eq!(text_at_range(header_text, value_definition.range), "value");
    assert!(
        macro_param_definition_at(&db, HEADER, value_definition.range.end()).unwrap().is_none()
    );

    let value_reference = macro_param_reference_definitions_at(
        &db,
        HEADER,
        offset_after(header_text, "SHIFT(value, amount) (("),
    )
    .unwrap()
    .unwrap();
    assert_eq!(text_at_range(header_text, value_reference.range), "value");
    assert!(
        macro_param_reference_definitions_at(&db, HEADER, value_reference.range.end())
            .unwrap()
            .is_none()
    );
    assert!(value_reference.definitions.iter().any(|definition| {
        definition.param_index == value_definition.param_index
            && text_at_range(header_text, definition.range) == "value"
    }));

    let refs = macro_param_references(&db, HEADER, &value_definition).unwrap().references;
    assert!(refs.iter().any(|reference| {
        reference.file_id == HEADER && text_at_range(header_text, reference.range) == "value"
    }));
    assert!(!refs.iter().any(|reference| text_at_range(header_text, reference.range) == "amount"));
}

#[test]
fn preproc_header_reference_reports_all_including_context_definitions() {
    let root_text = r#"`define WIDTH 8
`include "defs.vh"
`undef WIDTH
`define WIDTH 16
`include "defs.vh"
"#;
    let header_text = "localparam int W = `WIDTH;\n";
    let db = db_with_files(root_text, header_text);

    let definitions =
        macro_reference_definitions_at(&db, HEADER, offset(header_text, "WIDTH")).unwrap().unwrap();

    assert_eq!(text_at_range(header_text, definitions.range), "`WIDTH");
    assert_eq!(definitions.definitions.len(), 2);
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP
            && definition.name_range.start() == offset_after_n(root_text, "`define ", 0)
    }));
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP
            && definition.name_range.start() == offset_after_n(root_text, "`define ", 1)
    }));
}

#[test]
fn preproc_header_macro_body_reference_reports_all_expansion_context_definitions() {
    let root_text = r#"`define WIDTH 8
`include "defs.vh"
localparam int A = `USE_WIDTH;
`undef WIDTH
`define WIDTH 16
`include "defs.vh"
localparam int B = `USE_WIDTH;
"#;
    let header_text = "`define USE_WIDTH `WIDTH\n";
    let db = db_with_files(root_text, header_text);

    let definitions =
        macro_reference_definitions_at(&db, HEADER, offset_after(header_text, "USE_WIDTH `"))
            .unwrap()
            .unwrap();

    assert_eq!(text_at_range(header_text, definitions.range), "`WIDTH");
    assert_eq!(definitions.definitions.len(), 2);
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP
            && definition.name_range.start() == offset_after_n(root_text, "`define ", 0)
    }));
    assert!(definitions.definitions.iter().any(|definition| {
        definition.file_id == TOP
            && definition.name_range.start() == offset_after_n(root_text, "`define ", 1)
    }));
}

#[test]
fn preproc_macro_definition_at_only_hits_name_range() {
    let root_text = "`define HEADER_FLAG 1\n";
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    assert!(macro_definition_at(&db, TOP, offset(root_text, "`define")).unwrap().is_none());

    let definition =
        macro_definition_at(&db, TOP, offset(root_text, "HEADER_FLAG")).unwrap().unwrap();
    assert_eq!(text_at_range(root_text, definition.name_range), "HEADER_FLAG");
    assert!(macro_definition_at(&db, TOP, definition.name_range.end()).unwrap().is_none());
    assert_ne!(definition.directive_range, definition.name_range);
}

#[test]
fn preproc_ifndef_guard_reference_resolves_to_following_define() {
    let root_text = "`include \"defs.vh\"\n";
    let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
    let db = db_with_files(root_text, header_text);
    let resolution =
        macro_reference_definitions_at(&db, HEADER, offset(header_text, "HEADER_FLAG"))
            .unwrap()
            .unwrap();

    assert!(resolution.references.iter().any(|reference| reference.file_id == HEADER));
    let definition =
        resolution.definitions.iter().find(|definition| definition.file_id == HEADER).unwrap();
    assert_eq!(text_at_range(header_text, definition.name_range), "HEADER_FLAG");

    let refs = macro_references(&db, HEADER, definition).unwrap().references;
    assert!(refs.iter().any(|reference| {
        reference.file_id == HEADER
            && reference.range.start() == offset(header_text, "HEADER_FLAG")
            && text_at_range(header_text, reference.range) == "HEADER_FLAG"
    }));
}

#[test]
fn preproc_macro_references_in_range_includes_undefined_conditionals() {
    let root_text = r#"`define KNOWN 1
`ifdef UNKNOWN
`endif
`ifndef KNOWN
`endif
module top;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let references =
        macro_references_in_range(&db, TOP, TextRange::up_to(TextSize::of(root_text))).unwrap();

    let unknown = references
        .iter()
        .find(|reference| reference.name.as_str() == "UNKNOWN")
        .expect("undefined conditional macro reference should be present");
    assert_eq!(text_at_range(root_text, unknown.range), "UNKNOWN");
    assert!(matches!(unknown.resolution, MacroResolution::Undefined));

    let known = references
        .iter()
        .find(|reference| reference.name.as_str() == "KNOWN")
        .expect("resolved conditional macro reference should be present");
    assert_eq!(text_at_range(root_text, known.range), "KNOWN");
    assert!(matches!(known.resolution, MacroResolution::Resolved { .. }));
}

#[test]
fn preproc_project_header_guard_reference_is_indexed_without_include() {
    let root_text = "module top; endmodule\n";
    let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
    let db = db_with_files(root_text, header_text);
    let resolution =
        macro_reference_definitions_at(&db, HEADER, offset(header_text, "HEADER_FLAG"))
            .unwrap()
            .unwrap();

    assert!(resolution.references.iter().any(|reference| reference.file_id == HEADER));
    assert!(resolution.definitions.iter().any(|definition| {
        definition.file_id == HEADER
            && text_at_range(header_text, definition.name_range) == "HEADER_FLAG"
    }));
}
