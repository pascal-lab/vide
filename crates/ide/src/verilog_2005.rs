use std::{
    collections::HashMap,
    fmt::Write,
    path::{Path, PathBuf},
};

use hir::{
    base_db::{
        change::Change,
        preproc_index::MacroIncludeTarget,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        source_db::SourceDb,
        source_root::{SourceRoot, SourceRootId},
    },
    semantics::Semantics,
};
use insta::assert_snapshot;
use triomphe::Arc;
use utils::{
    lines::LineEnding,
    text_edit::{TextRange, TextSize},
};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use crate::{
    FilePosition, ScopeVisibility,
    analysis_host::AnalysisHost,
    completion::CompletionItem,
    db::root_db::RootDb,
    document_highlight::DocumentHighlightConfig,
    document_symbols::DocumentSymbol,
    folding_ranges::FoldingConfig,
    hover::{HoverConfig, HoverFormat},
    references::{ReferencesConfig, search::SearchScope},
    rename::{RenameConfig, RenameEditScope, RenameError},
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

fn setup_best_effort_with_path(text: &str, path: &str) -> (AnalysisHost, FileId, String) {
    let text = normalize_fixture_text(text);
    let file_id = FileId(0);
    let path = VfsPath::new_virtual_path(path.to_string());

    let mut file_set = FileSet::default();
    file_set.insert(file_id, path);
    let root = SourceRoot::new_best_effort_index(file_set);

    let mut change = Change::new();
    change.set_roots(vec![root]);
    change.add_changed_file(ChangedFile {
        file_id,
        change_kind: ChangeKind::Create(Arc::from(text.as_str()), LineEnding::Unix),
    });

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    (host, file_id, text)
}

#[test]
fn parsed_file_nodes_survive_parse_lru_eviction() {
    let mut file_set = FileSet::default();
    let files = [
        (FileId(0), "/a.sv", "module a;\n  wire x;\nendmodule\n"),
        (FileId(1), "/b.sv", "module b;\nendmodule\n"),
        (FileId(2), "/c.sv", "module c;\nendmodule\n"),
    ];

    let mut change = Change::new();
    for (file_id, path, text) in files {
        file_set.insert(file_id, VfsPath::new_virtual_path(path.to_owned()));
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(text), LineEnding::Unix),
        });
    }
    change.set_roots(vec![SourceRoot::new_local(file_set)]);

    let mut db = RootDb::new(Some(1));
    db.apply_change(change);

    let sema = Semantics::new(&db);
    let parsed_file = sema.parse_file(FileId(0));
    let root = parsed_file.root().expect("a.sv should parse");
    let child_count = root.child_count();
    assert!(child_count > 0);

    let _ = db.parse_src(FileId(1));
    let _ = db.parse_src(FileId(2));

    assert_eq!(root.child_count(), child_count);
    assert!(root.first_token().is_some());
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

fn setup_marked_with_predefines(
    text: &str,
    predefines: Vec<String>,
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

    let file_id = FileId(0);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new_virtual_path("/feature.v".to_owned()));

    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.set_project_config(Arc::new(ProjectConfig::new(
        vec![Some(CompilationProfileId(0))],
        vec![CompilationProfile {
            source_roots: vec![SourceRootId(0)],
            top_modules: Vec::new(),
            preprocess: PreprocessConfig { predefines, include_dirs: Vec::new() },
        }],
    )));
    change.add_changed_file(ChangedFile {
        file_id,
        change_kind: ChangeKind::Create(Arc::from(text.as_str()), LineEnding::Unix),
    });

    let mut host = AnalysisHost::default();
    host.apply_change(change);
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
    completion_labels_for_with_path(text, marker, "/feature.v")
}

fn completion_labels_for_with_path(text: &str, marker: &str, path: &str) -> Vec<String> {
    let (host, file_id, _clean_text, markers) = setup_marked_with_path(text, path);
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
fn best_effort_single_file_rename_updates_local_symbol() {
    let text = r#"
module top;
  logic sig;
  always_comb sig = sig;
endmodule
"#;
    let (host, file_id, clean_text) = setup_best_effort_with_path(text, "/feature.sv");
    let offset = TextSize::from(clean_text.find("sig = sig").expect("signal use") as u32);
    let config = RenameConfig::workspace(ScopeVisibility::Private)
        .with_edit_scope(RenameEditScope::SingleFile);

    let rename = host
        .make_analysis()
        .rename(FilePosition { file_id, offset }, config, "renamed_sig")
        .unwrap()
        .expect("best-effort local rename expected");
    let edit = rename.text_edits.get(&file_id).expect("rename should edit the current file");
    let mut renamed = clean_text;
    edit.apply(&mut renamed);
    assert!(renamed.contains("logic renamed_sig;"));
    assert!(renamed.contains("always_comb renamed_sig = renamed_sig;"));
}

#[test]
fn best_effort_single_file_rename_rejects_cross_file_symbol() {
    let child_text = "module child;\nendmodule\n";
    let top_text = "module top;\n  child u();\nendmodule\n";
    let child_file_id = FileId(0);
    let top_file_id = FileId(1);

    let mut file_set = FileSet::default();
    file_set.insert(child_file_id, VfsPath::new_virtual_path("/child.sv".to_owned()));
    file_set.insert(top_file_id, VfsPath::new_virtual_path("/top.sv".to_owned()));

    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_best_effort_index(file_set)]);
    change.add_changed_file(ChangedFile {
        file_id: child_file_id,
        change_kind: ChangeKind::Create(Arc::from(child_text), LineEnding::Unix),
    });
    change.add_changed_file(ChangedFile {
        file_id: top_file_id,
        change_kind: ChangeKind::Create(Arc::from(top_text), LineEnding::Unix),
    });

    let mut host = AnalysisHost::default();
    host.apply_change(change);

    let config = RenameConfig::workspace(ScopeVisibility::Private)
        .with_edit_scope(RenameEditScope::SingleFile);
    let offset = TextSize::from(top_text.find("child u").expect("module reference") as u32);
    let err = host
        .make_analysis()
        .rename(FilePosition { file_id: top_file_id, offset }, config, "renamed_child")
        .unwrap()
        .expect_err("cross-file best-effort rename should be rejected");

    assert!(matches!(err, RenameError::ProjectScopeRequired));
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
            RenameConfig::workspace(ScopeVisibility::Private),
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
fn package_imports_resolve_unqualified_names() {
    let text = r#"
package pkg;
  typedef enum logic [1:0] {
    IDLE
  } /*marker:type_def*/state_e;
  localparam int /*marker:param_def*/pkg_value = 1;
endpackage

module top;
  import pkg::*;
  /*marker:type_ref*/state_e state;
  initial state = /*marker:param_ref*/pkg_value;
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let type_nav = analysis
        .goto_definition(position(file_id, &markers, "type_ref"))
        .unwrap()
        .expect("imported typedef definition expected");
    assert!(
        type_nav.info.iter().any(|nav| nav.name.as_deref() == Some("state_e")),
        "imported typedef should resolve: {type_nav:?}"
    );

    let param_nav = analysis
        .goto_definition(position(file_id, &markers, "param_ref"))
        .unwrap()
        .expect("imported parameter definition expected");
    assert!(
        param_nav.info.iter().any(|nav| nav.name.as_deref() == Some("pkg_value")),
        "imported parameter should resolve: {param_nav:?}"
    );
}

#[test]
fn package_exports_resolve_reexported_names() {
    let text = r#"
package leaf_pkg;
  typedef enum logic [1:0] {
    IDLE
  } /*marker:type_def*/state_e;
  localparam int /*marker:param_def*/exported_value = 1;
endpackage

package mid_pkg;
  export leaf_pkg::state_e;
  export leaf_pkg::exported_value;
endpackage

package all_pkg;
  import leaf_pkg::exported_value;
  export *::*;
endpackage

module top;
  import mid_pkg::*;
  /*marker:type_ref*/state_e state;
  initial state = /*marker:param_ref*/exported_value;
  initial state = mid_pkg::/*marker:scoped_ref*/exported_value;
  import all_pkg::exported_value;
  initial state = /*marker:export_all_ref*/exported_value;
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    for marker in ["type_ref"] {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} definition expected"));
        assert!(
            nav.info.iter().any(|nav| nav.name.as_deref() == Some("state_e")),
            "{marker} should resolve to exported typedef: {nav:?}"
        );
    }

    for marker in ["param_ref", "scoped_ref", "export_all_ref"] {
        let nav = analysis
            .goto_definition(position(file_id, &markers, marker))
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} definition expected"));
        assert!(
            nav.info.iter().any(|nav| nav.name.as_deref() == Some("exported_value")),
            "{marker} should resolve to exported parameter: {nav:?}"
        );
    }
}

#[test]
fn interface_ports_resolve_members_and_modports() {
    let text = r#"
interface /*marker:if_def*/bus_if;
  logic /*marker:req_def*/req;
  logic /*marker:gnt_def*/gnt;
  modport /*marker:master_def*/master(input gnt, output req);
endinterface

module top(/*marker:if_ref*/bus_if./*marker:modport_ref*/master bus);
  assign bus./*marker:req_ref*/req = bus./*marker:gnt_ref*/gnt;
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    for (marker, expected) in
        [("if_ref", "bus_if"), ("modport_ref", "master"), ("req_ref", "req"), ("gnt_ref", "gnt")]
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
    for expected in ["bus_if", "req", "gnt", "master", "top"] {
        assert!(
            names.iter().any(|name| name == expected),
            "missing document symbol {expected:?}; got {names:?}"
        );
    }

    let hover = analysis
        .hover(
            position(file_id, &markers, "if_ref"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("interface hover expected");
    assert!(
        hover.info.as_str().contains("interface bus_if"),
        "interface hover should use interface signature: {}",
        hover.info.as_str()
    );
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
fn manifest_predefines_select_ide_ifdef_branch_for_navigation() {
    let text = r#"
module manifest_define_ctx;
`ifdef USE_IMPL
  logic /*marker:active_def*/branch_sig;
  assign /*marker:active_ref*/branch_sig = 1'b1;
`else
  logic /*marker:inactive_def*/branch_sig;
  assign /*marker:inactive_ref*/branch_sig = 1'b0;
`endif
  assign /*marker:query_ref*/branch_sig = 1'b1;
endmodule
"#;
    let (host, file_id, _clean_text, markers) =
        setup_marked_with_predefines(text, vec!["USE_IMPL=1".to_owned()]);
    let analysis = host.make_analysis();
    let branch_sig_len = TextSize::from("branch_sig".len() as u32);
    let active_def = TextRange::new(markers["active_def"], markers["active_def"] + branch_sig_len);
    let inactive_def =
        TextRange::new(markers["inactive_def"], markers["inactive_def"] + branch_sig_len);
    let active_ref = TextRange::new(markers["active_ref"], markers["active_ref"] + branch_sig_len);
    let inactive_ref =
        TextRange::new(markers["inactive_ref"], markers["inactive_ref"] + branch_sig_len);
    let query_ref = TextRange::new(markers["query_ref"], markers["query_ref"] + branch_sig_len);

    let nav = analysis
        .goto_definition(position(file_id, &markers, "query_ref"))
        .unwrap()
        .expect("branch_sig definition expected");
    assert!(
        nav.info.iter().any(|nav| nav.focus_range == Some(active_def)),
        "goto should resolve to the manifest-defined active branch: {nav:?}"
    );
    assert!(
        nav.info.iter().all(|nav| nav.focus_range != Some(inactive_def)),
        "goto must not resolve to the inactive branch: {nav:?}"
    );

    let refs = analysis
        .references(
            position(file_id, &markers, "query_ref"),
            ReferencesConfig::new(
                ScopeVisibility::Private,
                Some(SearchScope::single_file(file_id)),
            ),
        )
        .unwrap()
        .expect("branch_sig references expected");
    let ranges = refs
        .iter()
        .flat_map(|refs| refs.refs.get(&file_id).into_iter().flatten())
        .map(|(range, _)| *range)
        .collect::<Vec<_>>();
    assert!(ranges.contains(&active_ref), "active reference should be found: {ranges:?}");
    assert!(ranges.contains(&query_ref), "query reference should be found: {ranges:?}");
    assert!(
        !ranges.contains(&inactive_ref),
        "inactive reference should not be classified as the active definition: {ranges:?}"
    );
}

#[test]
fn manifest_predefines_feed_default_preproc_file_index() {
    let text = r#"
`ifdef USE_IMPL
`include "active.svh"
`else
`include "inactive.svh"
`endif
module manifest_preproc_index_ctx;
endmodule
"#;
    let (host, file_id, _clean_text, _markers) =
        setup_marked_with_predefines(text, vec!["USE_IMPL=1".to_owned()]);

    let index = host.raw_db().preproc_file_index(file_id);
    let literal_include_paths = index
        .includes
        .iter()
        .filter_map(|include| match &include.target {
            MacroIncludeTarget::Literal { path, .. } => Some(path.as_str()),
            MacroIncludeTarget::Token { .. } => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(literal_include_paths, vec!["active.svh"]);
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
            RenameConfig::workspace(ScopeVisibility::Private),
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
            RenameConfig::workspace(ScopeVisibility::Private),
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
fn verilog_2005_hover_renders_side_comment_from_trivia() {
    let text = r#"
module side_comment_ctx;
  wire /*marker:sig_def*/sig; // side comment from trivia
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let hover = host
        .make_analysis()
        .hover(
            position(file_id, &markers, "sig_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("signal hover expected");

    assert!(
        hover.info.as_str().contains("side comment from trivia"),
        "hover should render the declaration side comment: {}",
        hover.info.as_str()
    );
}

#[test]
fn verilog_2005_hover_after_truncation_uses_current_syntax_context() {
    let full = "module\t/*marker:name*/axi_addr_miter(i_last_addr, i_size, i_burst, i_len); // full declaration\nendmodule\n";
    let (mut host, file_id, _clean_text, markers) = setup_marked(full);

    let mut change = Change::new();
    change.add_changed_file(ChangedFile {
        file_id,
        change_kind: ChangeKind::Modify(
            Arc::from("module\taxi_addr_miter(i_last_addr, i_size, i_burst, i_len);"),
            LineEnding::Unix,
        ),
    });
    host.apply_change(change);

    let hover = host
        .make_analysis()
        .hover(position(file_id, &markers, "name"), HoverConfig { format: HoverFormat::PlainText })
        .unwrap()
        .expect("truncated module hover expected");

    assert!(
        !hover.info.as_str().contains("full declaration"),
        "hover should not render stale side comments: {}",
        hover.info.as_str()
    );
}

#[test]
fn verilog_2005_hover_uses_symbol_specific_renderers() {
    let text = r#"
module /*marker:module_def*/child #(parameter WIDTH = 8) (
  input wire /*marker:port_def*/clk
);
  localparam /*marker:param_def*/DEPTH = WIDTH + 1;
  reg data_valid;
  reg sink;
  initial sink = data_valid;
  task automatic /*marker:task_def*/drive(input reg [3:0] value);
  endtask
  function [3:0] /*marker:func_def*/add1(input [3:0] value);
    begin
      add1 = value + 1'b1;
    end
  endfunction
endmodule

module top;
  /*marker:module_ref*/child /*marker:instance_ref*/u_child(.clk());
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let module_hover = analysis
        .hover(
            position(file_id, &markers, "module_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("module hover expected");
    assert!(
        module_hover.info.as_str().contains(
            "module child #(\n    parameter logic WIDTH = 8\n) (\n    input wire logic clk\n)"
        ),
        "module hover should use module-specific renderer: {}",
        module_hover.info.as_str()
    );

    let inst_module_hover = analysis
        .hover(
            position(file_id, &markers, "module_ref"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("instantiated module hover expected");
    assert!(
        inst_module_hover.info.as_str().contains(
            "module child #(\n    parameter logic WIDTH = 8\n) (\n    input wire logic clk\n)"
        ),
        "instantiation module name hover should reuse module signature: {}",
        inst_module_hover.info.as_str()
    );

    let instance_hover = analysis
        .hover(
            position(file_id, &markers, "instance_ref"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("instance hover expected");
    assert!(
        instance_hover.info.as_str().contains("instance u_child of child")
            && instance_hover.info.as_str().contains(
                "module child #(\n    parameter logic WIDTH = 8\n) (\n    input wire logic clk\n)"
            ),
        "instance hover should include the target module signature: {}",
        instance_hover.info.as_str()
    );

    let port_hover = analysis
        .hover(
            position(file_id, &markers, "port_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("port hover expected");
    assert!(
        port_hover.info.as_str().contains("input wire logic clk"),
        "port hover should use port-specific renderer: {}",
        port_hover.info.as_str()
    );
    assert!(
        port_hover.info.as_str().contains("---------"),
        "port hover should separate signature and container: {}",
        port_hover.info.as_str()
    );

    let param_hover = analysis
        .hover(
            position(file_id, &markers, "param_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("parameter hover expected");
    assert!(
        param_hover.info.as_str().contains("parameter logic DEPTH = WIDTH + 1"),
        "parameter hover should use parameter-specific renderer: {}",
        param_hover.info.as_str()
    );
    assert!(
        param_hover.info.as_str().contains("---------"),
        "parameter hover should separate signature and container: {}",
        param_hover.info.as_str()
    );

    let task_hover = analysis
        .hover(
            position(file_id, &markers, "task_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("task hover expected");
    assert!(
        task_hover.info.as_str().contains("task drive(")
            && task_hover.info.as_str().contains("value"),
        "task hover should use subroutine-specific renderer: {}",
        task_hover.info.as_str()
    );

    let func_hover = analysis
        .hover(
            position(file_id, &markers, "func_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("function hover expected");
    assert!(
        func_hover.info.as_str().contains("function")
            && func_hover.info.as_str().contains("add1(")
            && func_hover.info.as_str().contains("value"),
        "function hover should use subroutine-specific renderer: {}",
        func_hover.info.as_str()
    );
}

#[test]
fn ambiguous_instantiation_hover_lists_locations_without_expanding_signatures() {
    let text = r#"
module child(input logic a);
endmodule

module child(output logic y);
endmodule

module top;
  /*marker:child_ref*/child u();
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let hover = analysis
        .hover(
            position(file_id, &markers, "child_ref"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("ambiguous module hover expected");
    let info = hover.info.as_str();

    assert!(
        info.contains("Ambiguous reference"),
        "ambiguous hover should identify the ambiguity: {info}"
    );
    assert!(
        info.contains("feature.v:2") && info.contains("feature.v:5"),
        "ambiguous hover should list declaration locations: {info}"
    );
    assert!(
        !info.contains("input logic a") && !info.contains("output logic y"),
        "ambiguous hover should not expand candidate signatures: {info}"
    );
}

#[test]
fn verilog_2005_module_definition_names_support_references() {
    let text = r#"
module /*marker:module_def*/mux2X1(in0, in1, sel, out);
  input in0, in1;
  input sel;
  output out;
  assign out = sel ? in1 : in0;
endmodule

module top;
  wire out;
  /*marker:module_ref*/mux2X1 u_mux(1'b0, 1'b1, 1'b0, out);
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let analysis = host.make_analysis();

    let hover = analysis
        .hover(
            position(file_id, &markers, "module_def"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("module definition hover expected");
    assert!(
        hover.info.as_str().contains("module mux2X1"),
        "module definition hover should resolve to the declared module: {}",
        hover.info.as_str()
    );

    let refs = analysis
        .references(
            position(file_id, &markers, "module_def"),
            ReferencesConfig::new(ScopeVisibility::Public, Some(SearchScope::single_file(file_id))),
        )
        .unwrap()
        .expect("module definition references expected");
    let def_count: usize = refs.iter().map(|refs| refs.def.as_ref().map_or(0, Vec::len)).sum();
    let ref_count: usize = refs.iter().flat_map(|refs| refs.refs.values()).map(Vec::len).sum();
    assert_eq!(def_count, 1, "module definition should be returned as the declaration");
    assert_eq!(ref_count, 1, "module instantiation should be returned as a reference: {refs:?}");

    let nav = analysis
        .goto_definition(position(file_id, &markers, "module_ref"))
        .unwrap()
        .expect("module reference definition expected");
    assert!(
        nav.info.iter().any(|target| target.name.as_deref() == Some("mux2X1")),
        "module reference should still resolve to the declaration: {nav:?}"
    );
}

#[test]
fn verilog_2005_hover_covers_all_definition_kinds() {
    let (host, file_id, _clean_text, markers) = setup_marked(VERILOG_2005_NAV_TEXT);
    let analysis = host.make_analysis();

    for (marker, expected) in [
        ("module_ref", "module child"),
        ("port_ref", "input wire logic a"),
        ("sig_ref", "wire logic sig"),
        ("udp_ref", "primitive udp_and"),
        ("task_ref", "task do_task"),
        ("genvar_ref", "genvar"),
        ("gen_label", "generate g_loop"),
        ("specparam_ref", "specparam"),
        ("instance_ref", "instance u_child"),
        ("generate_ref", "generate g_loop"),
        ("lane_ref", "wire logic lane"),
        ("block_ref", "block blk"),
        ("config_def", "config cfg_top"),
    ] {
        let hover = analysis
            .hover(
                position(file_id, &markers, marker),
                HoverConfig { format: HoverFormat::PlainText },
            )
            .unwrap()
            .unwrap_or_else(|| panic!("{marker} hover expected"));
        assert!(
            hover.info.as_str().contains(expected),
            "{marker} hover should contain {expected:?}: {}",
            hover.info.as_str()
        );
    }
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
            RenameConfig::workspace(ScopeVisibility::Private),
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
fn verilog_2005_ansi_ports_inherit_implicit_header_type() {
    let text = r#"
module child(
    input rst,
    output io_vgaclk,
    output [7:0] a, b, c
);
endmodule

module top;
  child u(/*marker:rst*/rst, io_vgaclk, a, b, c);
endmodule
"#;
    let (host, file_id, _clean_text, markers) = setup_marked(text);
    let signature = host
        .make_analysis()
        .signature_help(
            position(file_id, &markers, "rst"),
            crate::signature_help::SignatureHelpConfig { params_only: false },
        )
        .unwrap()
        .expect("signature help expected for ordered port connection");

    assert_eq!(
        signature.label,
        "module child(input rst, output io_vgaclk, output [7:0] a, output [7:0] b, output [7:0] c)"
    );
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
            RenameConfig::workspace(ScopeVisibility::Private),
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
            hir_def::{
                module::ModuleId,
                stmt::{CaseItem, Stmt, StmtId, StmtKind},
            },
        };
        use la_arena::Arena;

        fn stmt_tree_has(
            db: &dyn HirDb,
            stmts: &Arena<Stmt>,
            stmt_id: StmtId,
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
                    CaseItem::Case { clause, .. } | CaseItem::Default(clause) => {
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
            stmts: &Arena<Stmt>,
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
            RenameConfig::workspace(ScopeVisibility::Private),
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
                RenameConfig::workspace(ScopeVisibility::Private),
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
        completion_labels_for_with_path("lib/*marker:library*/\n", "library", "/feature.map")
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
