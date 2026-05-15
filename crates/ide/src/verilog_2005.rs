use std::{
    collections::HashMap,
    fmt::Write,
    path::{Path, PathBuf},
};

use base_db::{change::Change, source_root::SourceRoot};
use insta::assert_snapshot;
use span::FilePosition;
use triomphe::Arc;
use utils::{
    lines::LineEnding,
    text_edit::{TextRange, TextSize},
};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use crate::{
    ScopeVisibility,
    analysis_host::AnalysisHost,
    completion::CompletionItem,
    document_highlight::DocumentHighlightConfig,
    document_symbols::DocumentSymbol,
    folding_ranges::FoldingConfig,
    hover::{HoverConfig, HoverFormat},
    references::{ReferencesConfig, search::SearchScope},
    rename::RenameConfig,
    semantic_tokens::{SemaTokenConfig, SemaTokenPortConfig},
    test_utils::normalize_fixture_text,
};

const VERILOG_2005_NAV_TEXT: &str = r#"
module child(input wire a, output wire y);
  wire child_net;
endmodule

primitive /*marker:udp_def*/udp_and(out, in);
  output out;
  input in;
  table
    1 : 1;
  endtable
endprimitive

module top(input wire clk);
  wire /*marker:sig_def*/sig;
  /*marker:module_ref*/child u_child(./*marker:port_ref*/a(/*marker:sig_ref*/sig), .y());
  /*marker:udp_ref*/udp_and u_udp(sig, clk);

  task automatic /*marker:task_def*/do_task;
    input reg t_in;
    begin
      sig = t_in;
    end
  endtask

  generate
    genvar /*marker:genvar_def*/i;
    for (/*marker:genvar_ref*/i = 0; i < 1; i = i + 1) begin : /*marker:gen_label*/g_loop
      wire lane;
    end
  endgenerate

  specify
    specparam /*marker:specparam_def*/T_SETUP = 1;
    (clk => sig) = T_SETUP;
  endspecify

  initial begin : blk
    /*marker:task_ref*/do_task(sig);
    sig = /*marker:specparam_ref*/T_SETUP;
    sig = /*marker:instance_ref*/u_child.y;
    sig = /*marker:generate_ref*/g_loop[0]./*marker:lane_ref*/lane;
    disable /*marker:block_ref*/blk;
  end
endmodule

config /*marker:config_def*/cfg_top;
  design work.top;
endconfig
"#;

fn setup(text: &str) -> (AnalysisHost, FileId) {
    setup_with_path(text, "/feature.v")
}

fn setup_with_path(text: &str, path: &str) -> (AnalysisHost, FileId) {
    let text = normalize_fixture_text(text);
    let file_id = FileId(0);
    let path = VfsPath::new_virtual_path(path.to_string());

    let mut file_set = FileSet::default();
    file_set.insert(file_id, path);
    let root = SourceRoot::new_local(file_set);

    let mut change = Change::new();
    change.set_roots(vec![root]);
    change.add_changed_file(ChangedFile {
        file_id,
        change_kind: ChangeKind::Create(Arc::from(text.as_str()), LineEnding::Unix),
    });

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    (host, file_id)
}

fn setup_marked(text: &str) -> (AnalysisHost, FileId, String, HashMap<String, TextSize>) {
    setup_marked_with_path(text, "/feature.v")
}

fn setup_marked_with_path(
    text: &str,
    path: &str,
) -> (AnalysisHost, FileId, String, HashMap<String, TextSize>) {
    let mut text = normalize_fixture_text(text);
    let mut markers = HashMap::new();
    let mut cursor = 0;
    let prefix = "/*marker:";

    while let Some(rel_start) = text[cursor..].find(prefix) {
        let start = cursor + rel_start;
        let name_start = start + prefix.len();
        let Some(rel_end) = text[name_start..].find("*/") else {
            panic!("unterminated marker in fixture");
        };
        let name_end = name_start + rel_end;
        let name = text[name_start..name_end].to_string();
        let end = name_end + 2;
        text.replace_range(start..end, "");
        markers.insert(name, TextSize::from(start as u32));
        cursor = start;
    }

    let (host, file_id) = setup_with_path(&text, path);
    (host, file_id, text, markers)
}

fn position(file_id: FileId, markers: &HashMap<String, TextSize>, name: &str) -> FilePosition {
    FilePosition {
        file_id,
        offset: *markers.get(name).unwrap_or_else(|| panic!("missing marker {name:?}")),
    }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/verilog_2005/fixtures")
}

fn collect_fixture_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read fixtures dir {dir:?}: {err}"))
        .map(|entry| entry.unwrap_or_else(|err| panic!("failed to read fixture entry: {err}")))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_fixture_paths(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "v") {
            out.push(path);
        }
    }
}

fn expected_symbols(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("// expect-symbol: "))
        .map(str::to_string)
        .collect()
}

fn flatten_symbols(symbols: &[DocumentSymbol], out: &mut Vec<String>) {
    for symbol in symbols {
        out.push(symbol.name.clone());
        flatten_symbols(&symbol.children, out);
    }
}

fn collect_symbol_lines(symbols: &[DocumentSymbol], indent: usize, out: &mut Vec<String>) {
    for symbol in symbols {
        out.push(format!(
            "{}{} {:?} container={:?}",
            "  ".repeat(indent),
            symbol.name,
            symbol.kind,
            symbol.container_name
        ));
        collect_symbol_lines(&symbol.children, indent + 1, out);
    }
}

fn completion_labels(items: Vec<CompletionItem>) -> Vec<String> {
    items.into_iter().map(|item| format!("{} {:?}", item.label, item.kind)).collect()
}

fn completion_labels_for(text: &str, marker: &str) -> Vec<String> {
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    host.make_analysis()
        .completions_with_trigger(position(file_id, &markers, marker), None)
        .map(completion_labels)
        .unwrap()
}

#[test]
fn verilog_2005_feature_matrix_lsp_requests_do_not_panic() {
    let mut paths = Vec::new();
    collect_fixture_paths(&fixtures_dir(), &mut paths);
    assert!(!paths.is_empty(), "no Verilog-2005 feature fixtures found");

    for path in paths {
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
        let text = normalize_fixture_text(&text);
        let expected_symbols = expected_symbols(&text);
        let (host, file_id) = setup(&text);
        let analysis = host.make_analysis();
        let full_range = TextRange::up_to(utils::text_edit::TextSize::of(text.as_str()));

        let parse_diagnostics = analysis
            .parse_diagnostics(file_id)
            .unwrap_or_else(|_| panic!("parse diagnostics cancelled for {path:?}"));
        assert!(
            parse_diagnostics.is_empty(),
            "Verilog-2005 fixture should parse cleanly for {path:?}: {parse_diagnostics:?}"
        );

        let symbols = analysis
            .document_symbol(file_id)
            .unwrap_or_else(|_| panic!("document symbols cancelled for {path:?}"));
        let mut names = Vec::new();
        flatten_symbols(&symbols, &mut names);
        for expected in expected_symbols {
            assert!(
                names.iter().any(|name| name == &expected),
                "missing symbol {expected:?} in {path:?}; got {names:?}"
            );
        }

        analysis
            .semantic_tokens(
                file_id,
                SemaTokenConfig { port: SemaTokenPortConfig { clk_rst: false, io: false } },
                Some(full_range),
            )
            .unwrap_or_else(|_| panic!("semantic tokens cancelled for {path:?}"));

        analysis
            .folding_ranges(file_id, &FoldingConfig { line_fold_only: false })
            .unwrap_or_else(|_| panic!("folding ranges cancelled for {path:?}"));
    }
}

#[test]
fn verilog_2005_navigation_rename_hover_and_completion_smoke() {
    let text = r#"
module child(input wire a, output wire y);
endmodule

primitive udp_and(out, in);
  output out;
  input in;
  table
    1 : 1;
  endtable
endprimitive

module top(input wire clk);
  wire /*marker:sig_def*/sig;
  child /*marker:child_inst*/u_child(.a(/*marker:sig_ref*/sig), .y());

  task automatic /*marker:task_def*/do_task;
    input reg t_in;
    begin
      sig = t_in;
    end
  endtask

  generate
    genvar i;
    for (i = 0; i < 1; i = i + 1) begin : /*marker:gen_label*/g_loop
      wire lane;
    end
  endgenerate

  specify
    specparam /*marker:specparam_def*/T_SETUP = 1;
  endspecify

  initial begin : blk
    /*marker:task_ref*/do_task(sig);
    sig = /*marker:generate_ref*/g_loop[0]./*marker:lane_ref*/lane;
  end
endmodule

config /*marker:config_def*/cfg_top;
  design work.top;
endconfig
"#;
    let (host, file_id, clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let sig_nav = analysis
        .goto_definition(position(file_id, &markers, "sig_ref"))
        .unwrap()
        .expect("sig definition expected");
    assert!(
        sig_nav.info.iter().any(|nav| nav.name.as_deref() == Some("sig")),
        "sig definition should be reachable: {sig_nav:?}"
    );

    let task_nav = analysis
        .goto_definition(position(file_id, &markers, "task_ref"))
        .unwrap()
        .expect("task definition expected");
    assert!(
        task_nav.info.iter().any(|nav| nav.name.as_deref() == Some("do_task")),
        "task definition should be reachable: {task_nav:?}"
    );

    let refs = analysis
        .references(
            position(file_id, &markers, "sig_ref"),
            ReferencesConfig::new(
                ScopeVisibility::Private,
                Some(SearchScope::single_file(file_id)),
            ),
        )
        .unwrap()
        .expect("sig references expected");
    let ref_count: usize = refs.iter().flat_map(|refs| refs.refs.values()).map(Vec::len).sum();
    assert!(ref_count >= 2, "sig references should include procedural uses: {refs:?}");

    let rename = analysis
        .rename(
            position(file_id, &markers, "sig_ref"),
            RenameConfig { scope_visibility: ScopeVisibility::Private },
            "renamed_sig",
        )
        .unwrap()
        .expect("sig rename expected");
    let edit = rename.text_edits.get(&file_id).expect("rename should edit fixture file");
    let mut renamed = clean_text.clone();
    edit.apply(&mut renamed);
    assert!(renamed.contains("wire renamed_sig;"));
    assert!(renamed.contains(".a(renamed_sig)"));
    assert!(renamed.contains("do_task(renamed_sig)"));

    let generate_nav = analysis
        .goto_definition(position(file_id, &markers, "generate_ref"))
        .unwrap()
        .expect("generate scope definition expected");
    assert!(
        generate_nav.info.iter().any(|nav| nav.name.as_deref() == Some("g_loop")),
        "generate scope should be reachable: {generate_nav:?}"
    );

    let lane_nav = analysis
        .goto_definition(position(file_id, &markers, "lane_ref"))
        .unwrap()
        .expect("generate member definition expected");
    assert!(
        lane_nav.info.iter().any(|nav| nav.name.as_deref() == Some("lane")),
        "generate member should resolve through generate scope: {lane_nav:?}"
    );

    let symbols = analysis.document_symbol(file_id).unwrap();
    let mut names = Vec::new();
    flatten_symbols(&symbols, &mut names);
    for expected in [
        "child", "udp_and", "top", "sig", "u_child", "do_task", "i", "g_loop", "T_SETUP", "cfg_top",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "missing document symbol {expected:?}; got {names:?}"
        );
    }
}

#[test]
fn verilog_2005_completion_keywords_cover_core_contexts() {
    let text = r#"
con/*marker:top_level*/

module completion_ctx(input wire clk);
  gen/*marker:module_item*/

  task automatic worker;
    begin
      ca/*marker:task_body*/
    end
  endtask
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();
    let labels = |marker: &str| {
        analysis
            .completions_with_trigger(position(file_id, &markers, marker), None)
            .unwrap()
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>()
    };

    let top_level = labels("top_level");
    assert!(top_level.iter().any(|label| label == "config"), "{top_level:?}");

    let module_item = labels("module_item");
    assert!(module_item.iter().any(|label| label == "generate"), "{module_item:?}");
    assert!(module_item.iter().any(|label| label == "genvar"), "{module_item:?}");

    let task_body = labels("task_body");
    assert!(task_body.iter().any(|label| label == "case"), "{task_body:?}");
}

#[test]
fn verilog_2005_genvar_declaration_lowers_without_fallback() {
    let text = r#"
module genvar_ctx;
  generate
    genvar i, j;
    for (i = 0; i < 1; i = i + 1) begin : g_loop
      wire lane;
    end
  endgenerate
endmodule
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().parse_diagnostics(file_id).unwrap();
    assert!(diagnostics.is_empty(), "fixture should parse cleanly: {diagnostics:?}");
}

#[test]
fn verilog_2005_conditional_generate_lowers_without_fallback() {
    let text = r#"
module conditional_generate_ctx;
  parameter /*marker:param_def*/P = 1;
  wire use_if, use_case, use_single_if, use_single_case;

  generate
    if (P) begin : /*marker:if_scope_def*/g_if
      wire /*marker:lane_if_def*/lane_if;
    end else begin : g_else
      wire lane_else;
    end

    case (P)
      1: begin : /*marker:case_scope_def*/g_case
        wire /*marker:lane_case_def*/lane_case;
      end
      default: begin : g_default
        wire lane_default;
      end
    endcase

    if (P) assign use_single_if = /*marker:single_if_param_ref*/P;
    case (P)
      default: assign use_single_case = /*marker:single_case_param_ref*/P;
    endcase
  endgenerate

  assign use_if = /*marker:if_scope_ref*/g_if./*marker:lane_if_ref*/lane_if;
  assign use_case = /*marker:case_scope_ref*/g_case./*marker:lane_case_ref*/lane_case;
endmodule
"#;
    let (host, file_id, clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    for (marker, expected) in [
        ("if_scope_ref", "g_if"),
        ("lane_if_ref", "lane_if"),
        ("case_scope_ref", "g_case"),
        ("lane_case_ref", "lane_case"),
        ("single_if_param_ref", "P"),
        ("single_case_param_ref", "P"),
    ] {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} definition expected"));
        assert!(
            nav.info.iter().any(|nav| nav.name.as_deref() == Some(expected)),
            "{marker} should resolve to {expected:?}: {nav:?}"
        );
    }

    let rename = analysis
        .rename(
            position(file_id, &markers, "case_scope_ref"),
            RenameConfig { scope_visibility: ScopeVisibility::Private },
            "renamed_g_case",
        )
        .unwrap()
        .expect("case generate scope rename expected");
    let edit = rename.text_edits.get(&file_id).expect("rename should edit fixture file");
    let mut renamed = clean_text;
    edit.apply(&mut renamed);
    assert!(renamed.contains("begin : renamed_g_case"));
    assert!(renamed.contains("assign use_case = renamed_g_case.lane_case;"));
}

#[test]
fn verilog_2005_direct_generate_lowers_without_fallback() {
    let text = r#"
module direct_generate_ctx;
  parameter P = 1;
  genvar /*marker:genvar_def*/i;
  wire use_if, use_loop, use_case;

  if (P) begin : /*marker:if_scope_def*/dg_if
    wire /*marker:lane_if_def*/lane_if;
  end

  for (/*marker:genvar_ref*/i = 0; i < 1; i = i + 1) begin : /*marker:loop_scope_def*/dg_loop
    wire /*marker:lane_loop_def*/lane_loop;
  end

  case (P)
    1: begin : /*marker:case_scope_def*/dg_case
      wire /*marker:lane_case_def*/lane_case;
    end
  endcase

  assign use_if = /*marker:if_scope_ref*/dg_if./*marker:lane_if_ref*/lane_if;
  assign use_loop = /*marker:loop_scope_ref*/dg_loop[0]./*marker:lane_loop_ref*/lane_loop;
  assign use_case = /*marker:case_scope_ref*/dg_case./*marker:lane_case_ref*/lane_case;
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    for (marker, expected) in [
        ("genvar_ref", "i"),
        ("if_scope_ref", "dg_if"),
        ("lane_if_ref", "lane_if"),
        ("loop_scope_ref", "dg_loop"),
        ("lane_loop_ref", "lane_loop"),
        ("case_scope_ref", "dg_case"),
        ("lane_case_ref", "lane_case"),
    ] {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} definition expected"));
        assert!(
            nav.info.iter().any(|nav| nav.name.as_deref() == Some(expected)),
            "{marker} should resolve to {expected:?}: {nav:?}"
        );
    }
}

#[test]
fn verilog_2005_generate_region_direct_items_lower_without_fallback() {
    let text = r#"
module child(input wire a, output wire y);
  assign y = a;
endmodule

module generate_region_direct_ctx(input wire a, output wire y);
  generate
    localparam /*marker:param_def*/P_DLY = 1;
    wire /*marker:wire_def*/direct_wire;
    assign direct_wire = a;
    child /*marker:inst_def*/u_direct(.a(/*marker:wire_ref*/direct_wire), .y());
    initial begin
      $display(/*marker:param_ref*/P_DLY);
    end
  endgenerate

  assign y = /*marker:inst_ref*/u_direct.y | direct_wire;
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let parse_diagnostics = analysis.parse_diagnostics(file_id).unwrap();
    assert!(
        parse_diagnostics.is_empty(),
        "fixture should be valid Verilog-2005: {parse_diagnostics:?}"
    );

    for (marker, expected) in
        [("wire_ref", "direct_wire"), ("param_ref", "P_DLY"), ("inst_ref", "u_direct")]
    {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} definition expected"));
        assert!(
            nav.info.iter().any(|nav| nav.name.as_deref() == Some(expected)),
            "{marker} should resolve to {expected:?}: {nav:?}"
        );
    }

    let symbols = analysis.document_symbol(file_id).unwrap();
    let mut names = Vec::new();
    flatten_symbols(&symbols, &mut names);
    for expected in ["P_DLY", "direct_wire", "u_direct"] {
        assert!(
            names.iter().any(|name| name == expected),
            "missing document symbol {expected:?}; got {names:?}"
        );
    }
}

#[test]
fn verilog_2005_generate_block_parameter_lowers_without_fallback() {
    let text = r#"
module generate_block_parameter_ctx(output wire y);
  generate
    if (1) begin : g_spec
      localparam /*marker:param_def*/P_LOCAL = 1;
      wire lane;
      assign lane = /*marker:param_ref*/P_LOCAL;
    end
  endgenerate

  assign y = g_spec.lane;
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let parse_diagnostics = analysis.parse_diagnostics(file_id).unwrap();
    assert!(
        parse_diagnostics.is_empty(),
        "fixture should be valid Verilog-2005: {parse_diagnostics:?}"
    );

    let nav = analysis
        .goto_definition(position(file_id, &markers, "param_ref"))
        .unwrap()
        .expect("generate block localparam definition expected");
    assert!(
        nav.info.iter().any(|nav| nav.name.as_deref() == Some("P_LOCAL")),
        "generate block localparam should resolve locally: {nav:?}"
    );
}

#[test]
fn verilog_2005_library_map_declaration_lowers_without_fallback() {
    let text = r#"
library /*marker:library_def*/work "*.v";
include "vendor.map";
"#;
    let (host, file_id, _clean_text, markers) = setup_marked_with_path(text, "/feature.map");
    let analysis = host.make_analysis();

    let parse_diagnostics = analysis.parse_diagnostics(file_id).unwrap();
    assert!(
        parse_diagnostics.is_empty(),
        "library map fixture should parse cleanly: {parse_diagnostics:?}"
    );

    let symbols = analysis.document_symbol(file_id).unwrap();
    let mut names = Vec::new();
    flatten_symbols(&symbols, &mut names);
    assert!(names.iter().any(|name| name == "work"), "missing library symbol: {names:?}");

    let nav = analysis
        .goto_definition(position(file_id, &markers, "library_def"))
        .unwrap()
        .expect("library definition expected");
    assert!(
        nav.info.iter().any(|nav| nav.name.as_deref() == Some("work")),
        "library declaration should resolve as a real definition: {nav:?}"
    );

    let declaration = analysis
        .goto_declaration(position(file_id, &markers, "library_def"))
        .unwrap()
        .expect("library declaration expected");
    assert!(
        declaration.info.iter().any(|nav| nav.name.as_deref() == Some("work")),
        "library declaration should navigate without fallback: {declaration:?}"
    );

    let hover = analysis
        .hover(
            position(file_id, &markers, "library_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap();
    assert!(hover.is_some(), "library declaration hover should not panic or disappear");

    let highlights = analysis
        .document_highlight(
            position(file_id, &markers, "library_def"),
            DocumentHighlightConfig { scope_visibility: ScopeVisibility::Private },
        )
        .unwrap()
        .expect("library declaration highlights expected");
    assert!(!highlights.is_empty(), "library declaration should highlight its definition");

    let refs = analysis
        .references(
            position(file_id, &markers, "library_def"),
            ReferencesConfig::new(
                ScopeVisibility::Private,
                Some(SearchScope::single_file(file_id)),
            ),
        )
        .unwrap()
        .expect("library references expected");
    assert!(!refs.is_empty(), "library declaration should participate in references");

    let rename = analysis
        .rename(
            position(file_id, &markers, "library_def"),
            RenameConfig { scope_visibility: ScopeVisibility::Private },
            "renamed_work",
        )
        .unwrap()
        .expect("library rename expected");
    let edit = rename.text_edits.get(&file_id).expect("rename should edit library map");
    let mut renamed = _clean_text.clone();
    edit.apply(&mut renamed);
    assert!(
        renamed.contains("library renamed_work"),
        "library rename should rewrite declaration: {renamed}"
    );

    analysis.selection_ranges(position(file_id, &markers, "library_def")).unwrap();
    analysis.completions_with_trigger(position(file_id, &markers, "library_def"), None).unwrap();
}

#[test]
fn verilog_2005_block_parameter_declarations_lower_without_fallback() {
    let text = r#"
module block_param_ctx;
  initial begin : blk
    parameter /*marker:block_width_def*/WIDTH = 1;
    localparam DEPTH = /*marker:block_width_ref*/WIDTH + 1;
    reg [WIDTH:0] value;
    value = /*marker:block_depth_ref*/DEPTH;
  end

  function integer calc;
    parameter /*marker:func_base_def*/BASE = 1;
    integer tmp;
    begin
      tmp = /*marker:func_base_ref*/BASE;
      calc = tmp;
    end
  endfunction
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    for (marker, expected) in
        [("block_width_ref", "WIDTH"), ("block_depth_ref", "DEPTH"), ("func_base_ref", "BASE")]
    {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} definition expected"));
        assert!(
            nav.info.iter().any(|nav| nav.name.as_deref() == Some(expected)),
            "{marker} should resolve to {expected:?}: {nav:?}"
        );
    }
}

#[test]
fn verilog_2005_subroutine_port_declarations_resolve_locally() {
    let text = r#"
module subroutine_port_ctx(output reg [3:0] y);
  task drive;
    input [3:0] /*marker:task_value_def*/value;
    begin
      y = /*marker:task_value_ref*/value;
    end
  endtask

  function [3:0] add1;
    input [3:0] /*marker:func_value_def*/value;
    begin
      add1 = /*marker:func_value_ref*/value + 1'b1;
    end
  endfunction
endmodule
"#;
    let (host, file_id, clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    for marker in ["task_value_ref", "func_value_ref"] {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} definition expected"));
        assert!(
            nav.info.iter().any(|nav| nav.name.as_deref() == Some("value")),
            "{marker} should resolve to its local subroutine port declaration: {nav:?}"
        );
    }

    let rename = analysis
        .rename(
            position(file_id, &markers, "task_value_ref"),
            RenameConfig { scope_visibility: ScopeVisibility::Private },
            "drive_value",
        )
        .unwrap()
        .expect("task port rename expected");
    let edit = rename.text_edits.get(&file_id).expect("rename should edit fixture file");
    let mut renamed = clean_text;
    edit.apply(&mut renamed);
    assert!(renamed.contains("input [3:0] drive_value;"));
    assert!(renamed.contains("y = drive_value;"));
    assert!(renamed.contains("input [3:0] value;"));
    assert!(renamed.contains("add1 = value + 1'b1;"));
}

#[test]
fn verilog_2005_direct_generate_subroutine_resolves_locally() {
    let text = r#"
module direct_generate_subroutine_ctx;
  generate
    if (1)
      function integer f;
        input integer /*marker:arg_def*/arg;
        integer /*marker:local_def*/local_value;
        begin
          local_value = /*marker:arg_ref*/arg + /*marker:local_ref*/local_value;
        end
      endfunction
  endgenerate
endmodule
"#;
    let (host, file_id, _clean, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let parse_diagnostics = analysis.parse_diagnostics(file_id).unwrap();
    assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:?}");

    for (marker, expected) in [("arg_ref", "arg"), ("local_ref", "local_value")] {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} definition expected"));
        assert!(
            nav.info.iter().any(|target| target.name.as_deref() == Some(expected)),
            "{marker} should resolve to {expected}: {nav:?}"
        );
    }
}

#[test]
fn verilog_2005_specparam_declaration_lowers_without_fallback() {
    let text = r#"
module specparam_ctx(input wire a, output wire y);
  specify
    specparam T_SETUP = 1;
    (a => y) = T_SETUP;
  endspecify
endmodule
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().parse_diagnostics(file_id).unwrap();
    assert!(diagnostics.is_empty(), "fixture should parse cleanly: {diagnostics:?}");
}

#[test]
fn verilog_2005_specify_items_lower_without_fallback() {
    let text = r#"
module specify_ctx(input wire clk, input wire a, output wire y);
  specify
    specparam T_SETUP = 1;
    (a => y) = T_SETUP;
    if (clk) (a => y) = (1, 2, 3);
    ifnone (a => y) = 1;
    pulsestyle_onevent a;
    $setup(a, posedge clk, T_SETUP);
  endspecify
endmodule
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().parse_diagnostics(file_id).unwrap();
    assert!(diagnostics.is_empty(), "fixture should parse cleanly: {diagnostics:?}");
}

#[test]
fn verilog_2005_defparam_lowers_without_fallback() {
    let text = r#"
module child #(parameter WIDTH = 1) ();
endmodule

module defparam_ctx;
  child u_child();
  defparam u_child.WIDTH = 2;
endmodule
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().parse_diagnostics(file_id).unwrap();
    assert!(diagnostics.is_empty(), "fixture should parse cleanly: {diagnostics:?}");
}

#[test]
fn verilog_2005_config_declaration_lowers_without_fallback() {
    let text = r#"
module top;
endmodule

config cfg_top;
  design work.top;
endconfig
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().parse_diagnostics(file_id).unwrap();
    assert!(diagnostics.is_empty(), "fixture should parse cleanly: {diagnostics:?}");
}

#[test]
fn verilog_2005_udp_declaration_lowers_without_fallback() {
    let text = r#"
primitive udp_and(out, in);
  output out;
  input in;
  table
    1 : 1;
  endtable
endprimitive

module top(input wire clk);
  wire sig;
  udp_and u_udp(sig, clk);
endmodule
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().parse_diagnostics(file_id).unwrap();
    assert!(diagnostics.is_empty(), "fixture should parse cleanly: {diagnostics:?}");
}

#[test]
fn verilog_2005_event_trigger_statement_resolves_event_name() {
    let text = r#"
module event_ctx;
  event /*marker:event_def*/ev;
  initial begin
    -> /*marker:event_ref*/ev;
  end
endmodule
"#;
    let (host, file_id, clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let nav = analysis
        .goto_definition(position(file_id, &markers, "event_ref"))
        .unwrap()
        .expect("event definition expected");
    assert!(
        nav.info.iter().any(|nav| nav.name.as_deref() == Some("ev")),
        "event trigger should resolve to event declaration: {nav:?}"
    );

    let rename = analysis
        .rename(
            position(file_id, &markers, "event_ref"),
            RenameConfig { scope_visibility: ScopeVisibility::Private },
            "renamed_ev",
        )
        .unwrap()
        .expect("event rename expected");
    let edit = rename.text_edits.get(&file_id).expect("rename should edit fixture file");
    let mut renamed = clean_text;
    edit.apply(&mut renamed);
    assert!(renamed.contains("event renamed_ev;"));
    assert!(renamed.contains("-> renamed_ev;"));
}

#[test]
fn verilog_2005_procedural_statements_lower_references() {
    let text = r#"
module stmt_ctx;
  reg /*marker:sig_def*/sig;
  event ev;
  integer i;

  initial begin : blk
    sig = 0;
    #1 sig = sig;
    assign sig = sig;
    deassign sig;
    force sig = sig;
    release sig;
    wait (sig) sig = sig;
    -> ev;
    if (sig) sig = sig; else sig = sig;
    case (sig)
      1'b0: sig = sig;
      default: sig = sig;
    endcase
    forever sig = sig;
    repeat (1) sig = sig;
    while (sig) sig = sig;
    for (i = 0; i < 1; i = i + 1) sig = sig;
    begin
      sig = /*marker:sig_ref*/sig;
    end
    disable blk;
  end
endmodule
"#;
    let (host, file_id, clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    {
        use hir::{
            db::HirDb,
            file::HirFileId,
            hir_def::{module::ModuleId, stmt::StmtKind},
        };
        use la_arena::Arena;

        fn stmt_tree_has(
            db: &dyn HirDb,
            stmts: &Arena<hir::hir_def::stmt::Stmt>,
            stmt_id: hir::hir_def::stmt::StmtId,
            matches_kind: impl Copy + Fn(&StmtKind) -> bool,
        ) -> bool {
            let stmt = &stmts[stmt_id];
            if matches_kind(&stmt.kind) {
                return true;
            }

            match &stmt.kind {
                StmtKind::Block(info) => {
                    let block = db.block(info.block_id);
                    stmt_arena_has(db, &block.stmts, matches_kind)
                }
                StmtKind::TimingCtrl(_, stmt_id)
                | StmtKind::Forever(stmt_id)
                | StmtKind::DoWhile(stmt_id, _)
                | StmtKind::Repeat(_, stmt_id)
                | StmtKind::While(_, stmt_id)
                | StmtKind::Wait(_, stmt_id) => stmt_tree_has(db, stmts, *stmt_id, matches_kind),
                StmtKind::For { stmt, .. } => stmt_tree_has(db, stmts, *stmt, matches_kind),
                StmtKind::Cond { then_stmt, else_stmt, .. } => {
                    stmt_tree_has(db, stmts, *then_stmt, matches_kind)
                        || else_stmt
                            .is_some_and(|stmt_id| stmt_tree_has(db, stmts, stmt_id, matches_kind))
                }
                StmtKind::Case { items, .. } => items.iter().any(|item| match item {
                    hir::hir_def::stmt::CaseItem::Case { clause, .. }
                    | hir::hir_def::stmt::CaseItem::Default(clause) => {
                        stmt_tree_has(db, stmts, *clause, matches_kind)
                    }
                }),
                StmtKind::Empty
                | StmtKind::Expr(_)
                | StmtKind::Jump(_)
                | StmtKind::EventTrigger(_)
                | StmtKind::ProcAssign(_)
                | StmtKind::Disable(_) => false,
            }
        }

        fn stmt_arena_has(
            db: &dyn HirDb,
            stmts: &Arena<hir::hir_def::stmt::Stmt>,
            matches_kind: impl Copy + Fn(&StmtKind) -> bool,
        ) -> bool {
            stmts.iter().any(|(stmt_id, _)| stmt_tree_has(db, stmts, stmt_id, matches_kind))
        }

        let db = host.raw_db();
        let hir_file_id = HirFileId(file_id);
        let (hir_file, _) = db.hir_file_with_source_map(hir_file_id);
        let (local_module_id, _) =
            hir_file.modules.iter().next().expect("fixture should lower one module");
        let (module, _) = db.module_with_source_map(ModuleId::new(hir_file_id, local_module_id));
        assert!(
            stmt_arena_has(db, &module.stmts, |kind| matches!(kind, StmtKind::Repeat(_, _))),
            "repeat statements should lower distinctly from while statements"
        );
        assert!(
            stmt_arena_has(db, &module.stmts, |kind| matches!(kind, StmtKind::While(_, _))),
            "while statements should still lower as while statements"
        );
    }

    let nav = analysis
        .goto_definition(position(file_id, &markers, "sig_ref"))
        .unwrap()
        .expect("signal definition expected");
    assert!(
        nav.info.iter().any(|nav| nav.name.as_deref() == Some("sig")),
        "procedural statement references should resolve to the signal declaration: {nav:?}"
    );

    let refs = analysis
        .references(
            position(file_id, &markers, "sig_ref"),
            ReferencesConfig::new(
                ScopeVisibility::Private,
                Some(SearchScope::single_file(file_id)),
            ),
        )
        .unwrap()
        .expect("signal references expected");
    let ref_count: usize = refs.iter().flat_map(|refs| refs.refs.values()).map(Vec::len).sum();
    assert!(
        ref_count >= 30,
        "Verilog-2005 procedural statements should expose lowered expression refs: {refs:?}"
    );

    let rename = analysis
        .rename(
            position(file_id, &markers, "sig_ref"),
            RenameConfig { scope_visibility: ScopeVisibility::Private },
            "renamed_sig",
        )
        .unwrap()
        .expect("signal rename expected");
    let edit = rename.text_edits.get(&file_id).expect("rename should edit fixture file");
    let mut renamed = clean_text;
    edit.apply(&mut renamed);
    assert!(renamed.contains("reg renamed_sig;"));
    assert!(renamed.contains("wait (renamed_sig) renamed_sig = renamed_sig;"));
    assert!(renamed.contains("for (i = 0; i < 1; i = i + 1) renamed_sig = renamed_sig;"));
}

#[test]
fn verilog_2005_lsp_snapshots() {
    let (host, file_id, clean_text, markers) = setup_marked(VERILOG_2005_NAV_TEXT);
    let analysis = host.make_analysis();
    let full_range = TextRange::up_to(TextSize::of(clean_text.as_str()));
    let mut report = String::new();

    let symbols = analysis.document_symbol(file_id).unwrap();
    let mut symbol_lines = Vec::new();
    collect_symbol_lines(&symbols, 0, &mut symbol_lines);
    writeln!(&mut report, "# document symbols").unwrap();
    for line in symbol_lines {
        writeln!(&mut report, "{line}").unwrap();
    }

    let tokens = analysis
        .semantic_tokens(
            file_id,
            SemaTokenConfig { port: SemaTokenPortConfig { clk_rst: true, io: true } },
            Some(full_range),
        )
        .unwrap();
    writeln!(&mut report, "\n# semantic token classes").unwrap();
    for token in tokens {
        if !token.is_empty() {
            writeln!(&mut report, "{:?} {:?} {:?}", token.range, token.tag, token.mods).unwrap();
        }
    }

    writeln!(&mut report, "\n# navigation").unwrap();
    for marker in [
        "module_ref",
        "udp_ref",
        "port_ref",
        "sig_ref",
        "genvar_ref",
        "specparam_ref",
        "task_ref",
        "instance_ref",
        "generate_ref",
        "block_ref",
        "config_def",
    ] {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .map(|nav| {
                nav.info
                    .into_iter()
                    .map(|target| format!("{:?}:{:?}", target.name, target.kind))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        writeln!(&mut report, "{marker}: {nav:?}").unwrap();
    }

    writeln!(&mut report, "\n# references").unwrap();
    for marker in [
        "module_ref",
        "udp_ref",
        "port_ref",
        "sig_ref",
        "genvar_ref",
        "specparam_ref",
        "task_ref",
        "instance_ref",
        "generate_ref",
        "block_ref",
        "config_def",
    ] {
        let refs = analysis
            .references(
                position(file_id, &markers, marker),
                ReferencesConfig::new(
                    ScopeVisibility::Private,
                    Some(SearchScope::single_file(file_id)),
                ),
            )
            .unwrap()
            .unwrap_or_default();
        let def_count: usize = refs.iter().map(|refs| refs.def.as_ref().map_or(0, Vec::len)).sum();
        let ref_count: usize = refs.iter().flat_map(|refs| refs.refs.values()).map(Vec::len).sum();
        writeln!(&mut report, "{marker}: defs={def_count} refs={ref_count}").unwrap();
    }

    writeln!(&mut report, "\n# rename").unwrap();
    for (marker, new_name) in [
        ("module_ref", "renamed_child"),
        ("udp_ref", "renamed_udp"),
        ("port_ref", "renamed_a"),
        ("sig_ref", "renamed_sig"),
        ("genvar_ref", "renamed_i"),
        ("specparam_ref", "renamed_T_SETUP"),
        ("task_ref", "renamed_task"),
        ("instance_ref", "renamed_u_child"),
        ("generate_ref", "renamed_g_loop"),
        ("block_ref", "renamed_blk"),
        ("config_def", "renamed_cfg"),
    ] {
        let rename = analysis
            .rename(
                position(file_id, &markers, marker),
                RenameConfig { scope_visibility: ScopeVisibility::Private },
                new_name,
            )
            .unwrap()
            .unwrap_or_else(|err| panic!("{marker} rename expected: {err}"));
        let mut renamed = clean_text.clone();
        rename.text_edits.get(&file_id).unwrap().apply(&mut renamed);
        writeln!(&mut report, "{marker} -> {new_name}").unwrap();
        for line in renamed.lines().filter(|line| line.contains(new_name)) {
            writeln!(&mut report, "  {}", line.trim()).unwrap();
        }
    }

    writeln!(&mut report, "\n# completion").unwrap();
    writeln!(
        &mut report,
        "config: {:?}",
        completion_labels_for("con/*marker:config*/\n", "config")
    )
    .unwrap();
    writeln!(
        &mut report,
        "primitive_udp: {:?}",
        completion_labels_for("pri/*marker:primitive*/\n", "primitive")
    )
    .unwrap();
    writeln!(
        &mut report,
        "library: {:?}",
        completion_labels_for("lib/*marker:library*/\n", "library")
    )
    .unwrap();
    writeln!(
        &mut report,
        "generate: {:?}",
        completion_labels_for(
            "module completion_ctx; gen/*marker:generate*/ endmodule\n",
            "generate"
        )
    )
    .unwrap();
    writeln!(
        &mut report,
        "specify: {:?}",
        completion_labels_for(
            "module completion_ctx; spe/*marker:specify*/ endmodule\n",
            "specify"
        )
    )
    .unwrap();
    writeln!(
        &mut report,
        "task_body: {:?}",
        completion_labels_for(
            "module completion_ctx; task automatic worker; begin ca/*marker:task_body*/ end endtask endmodule\n",
            "task_body",
        )
    )
    .unwrap();

    assert_snapshot!("verilog_2005_lsp_snapshots", report);
}
