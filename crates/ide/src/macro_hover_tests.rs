use std::collections::HashMap;

use hir::base_db::{
    change::Change,
    project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
    source_root::{SourceRoot, SourceRootId},
};
use triomphe::Arc;
use utils::{lines::LineEnding, test_support::TestDir, text_edit::TextSize};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use crate::{
    FilePosition,
    analysis_host::AnalysisHost,
    hover::{HoverConfig, HoverFormat},
    test_utils::normalize_fixture_text,
};

struct PipelineMacroFixture {
    _dir: TestDir,
    host: AnalysisHost,
    top_file_id: FileId,
    header_file_id: FileId,
    top_markers: HashMap<String, TextSize>,
    header_markers: HashMap<String, TextSize>,
}

fn setup_pipeline_macro_fixture(predefines: Vec<&'static str>) -> PipelineMacroFixture {
    let dir = TestDir::new("pipeline-macro-hover");
    let rtl_dir = dir.path().join("rtl");
    let include_dir = dir.path().join("include");
    std::fs::create_dir_all(&rtl_dir).unwrap();
    std::fs::create_dir_all(&include_dir).unwrap();

    let top_path = rtl_dir.join("02_macro_hover_top.sv");
    let header_path = include_dir.join("pipeline_macros.svh");
    let marked_header_text = normalize_fixture_text(
        r#"
`ifndef PIPELINE_MACROS_SVH
`define PIPELINE_MACROS_SVH

`define DECL_PIPE(name, width) logic [(width)-1:0] name``_q
`define P/*marker:pipe_assign_def*/IPE_ASSIGN(name, next_value) \
  always_ff @(posedge clk_i or negedge rst_ni) begin \
    if (!rst_ni) begin \
      name``_q <= '0; \
    end else begin \
      name``_q <= (next_value); \
    end \
  end

`endif
"#,
    );
    let marked_top_text = normalize_fixture_text(
        r#"
`include "pipeline_macros.svh"

module macro_hover_top (
  input  logic                  clk_i,
  input  logic                  rst_ni,
  input  logic [`LANE_WIDTH-1:0] sample_i,
  output logic [`LANE_WIDTH-1:0] sample_o
);
  `DECL_PIPE(sample, `LANE_WIDTH);
  `DECL_PIPE(trace,  `LANE_WIDTH);

  `PIPE_ASSIGN(/*marker:trace_arg*/trace, sample_q ^ {{(`LANE_WIDTH-1){1'b0}}, 1'b1});

  assign sample_o = trace_q;
endmodule
"#,
    );
    let (header_text, header_markers) = strip_markers(marked_header_text);
    let (top_text, top_markers) = strip_markers(marked_top_text);
    std::fs::write(&top_path, &top_text).unwrap();
    std::fs::write(&header_path, &header_text).unwrap();

    let top_file_id = FileId(0);
    let header_file_id = FileId(1);
    let mut file_set = FileSet::default();
    file_set.insert(top_file_id, VfsPath::from(top_path));
    file_set.insert(header_file_id, VfsPath::from(header_path));

    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local_with_source_files(
        file_set,
        vec![top_file_id, header_file_id],
    )]);
    change.set_project_config(Arc::new(ProjectConfig::new(
        vec![Some(CompilationProfileId(0))],
        vec![CompilationProfile {
            source_roots: vec![SourceRootId(0)],
            top_modules: Vec::new(),
            preprocess: PreprocessConfig::with_predefine_strings(predefines, vec![include_dir]),
        }],
    )));
    change.add_changed_file(ChangedFile {
        file_id: top_file_id,
        change_kind: ChangeKind::Create(Arc::from(top_text.as_str()), LineEnding::Unix),
    });
    change.add_changed_file(ChangedFile {
        file_id: header_file_id,
        change_kind: ChangeKind::Create(Arc::from(header_text.as_str()), LineEnding::Unix),
    });

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    PipelineMacroFixture {
        _dir: dir,
        host,
        top_file_id,
        header_file_id,
        top_markers,
        header_markers,
    }
}

fn strip_markers(mut text: String) -> (String, HashMap<String, TextSize>) {
    let mut markers = HashMap::new();
    let mut cursor = 0;
    let prefix = "/*marker:";

    while let Some(rel_start) = text[cursor..].find(prefix) {
        let start = cursor + rel_start;
        let name_start = start + prefix.len();
        let rel_end = text[name_start..].find("*/").expect("unterminated marker in fixture");
        let name_end = name_start + rel_end;
        let name = text[name_start..name_end].to_string();
        let end = name_end + 2;
        text.replace_range(start..end, "");
        markers.insert(name, TextSize::from(start as u32));
        cursor = start;
    }

    (text, markers)
}

fn position(file_id: FileId, markers: &HashMap<String, TextSize>, name: &str) -> FilePosition {
    FilePosition {
        file_id,
        offset: *markers.get(name).unwrap_or_else(|| panic!("missing marker {name:?}")),
    }
}

#[test]
fn macro_definition_hover_preserves_multiline_source_layout() {
    let fixture = setup_pipeline_macro_fixture(Vec::new());
    let analysis = fixture.host.make_analysis();

    let hover = analysis
        .hover(
            position(fixture.header_file_id, &fixture.header_markers, "pipe_assign_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("PIPE_ASSIGN macro definition hover expected");
    let info = hover.info.as_str();
    let expected_definition = "`define PIPE_ASSIGN(name, next_value) \\
  always_ff @(posedge clk_i or negedge rst_ni) begin \\
    if (!rst_ni) begin \\
      name``_q <= '0; \\
    end else begin \\
      name``_q <= (next_value); \\
    end \\
  end";

    assert!(
        info.contains(expected_definition),
        "macro definition hover should preserve source line breaks and indentation: {info}"
    );
    assert!(
        !info.contains("always_ff @ ( posedge clk_i or negedge rst_ni )"),
        "macro definition hover should not reconstruct source by token joining: {info}"
    );
}

#[test]
fn macro_argument_hover_deduplicates_pasted_symbol_result() {
    let fixture = setup_pipeline_macro_fixture(vec!["LANE_WIDTH=12"]);
    let analysis = fixture.host.make_analysis();

    let hover = analysis
        .hover(
            position(fixture.top_file_id, &fixture.top_markers, "trace_arg"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("PIPE_ASSIGN trace argument hover expected");
    let info = hover.info.as_str();
    let trace_definition = "logic [12 - 1:0] trace_q";

    assert!(
        info.contains(trace_definition),
        "trace argument hover should resolve to the pasted trace_q definition: {info}"
    );
    assert_eq!(
        info.matches(trace_definition).count(),
        1,
        "trace argument hover should show the pasted trace_q definition once: {info}"
    );
}
