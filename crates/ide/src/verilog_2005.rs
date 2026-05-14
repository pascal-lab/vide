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
  endspecify

  initial begin : blk
    /*marker:task_ref*/do_task(sig);
    sig = /*marker:specparam_ref*/T_SETUP;
    sig = /*marker:instance_ref*/u_child.y;
    sig = /*marker:generate_ref*/g_loop[0].lane;
    disable /*marker:block_ref*/blk;
  end
endmodule

config /*marker:config_def*/cfg_top;
  design work.top;
endconfig
"#;

fn setup(text: &str) -> (AnalysisHost, FileId) {
    let text = normalize_fixture_text(text);
    let file_id = FileId(0);
    let path = VfsPath::new_virtual_path("/feature.v".to_string());

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

    let (host, file_id) = setup(&text);
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

        analysis
            .parse_diagnostics(file_id)
            .unwrap_or_else(|_| panic!("parse diagnostics cancelled for {path:?}"));

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

    let hover = analysis
        .hover(
            position(file_id, &markers, "gen_label"),
            HoverConfig { format: HoverFormat::PlainText },
        )
        .unwrap()
        .expect("generate label hover expected");
    assert!(
        hover.info.as_str().contains("recognized Verilog-2005 construct"),
        "opaque hover should explain limited semantic support: {:?}",
        hover.info
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
fn verilog_2005_genvar_declaration_is_not_model_limited() {
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
    let diagnostics = host.make_analysis().model_limit_diagnostics(file_id).unwrap();
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("GENVAR_DECLARATION")),
        "genvar declarations should lower as real HIR declarations, not opaque diagnostics: {diagnostics:?}"
    );
}

#[test]
fn verilog_2005_specparam_declaration_is_not_model_limited() {
    let text = r#"
module specparam_ctx(input wire a, output wire y);
  specify
    specparam T_SETUP = 1;
    (a => y) = T_SETUP;
  endspecify
endmodule
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().model_limit_diagnostics(file_id).unwrap();
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("SPECPARAM_DECLARATION")),
        "specparam declarations should lower as real HIR declarations, not opaque diagnostics: {diagnostics:?}"
    );
}

#[test]
fn verilog_2005_defparam_is_not_model_limited() {
    let text = r#"
module child #(parameter WIDTH = 1) ();
endmodule

module defparam_ctx;
  child u_child();
  defparam u_child.WIDTH = 2;
endmodule
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().model_limit_diagnostics(file_id).unwrap();
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("DEF_PARAM")),
        "defparam should lower as real HIR, not opaque diagnostics: {diagnostics:?}"
    );
}

#[test]
fn verilog_2005_config_declaration_is_not_model_limited() {
    let text = r#"
module top;
endmodule

config cfg_top;
  design work.top;
endconfig
"#;
    let (host, file_id) = setup(text);
    let diagnostics = host.make_analysis().model_limit_diagnostics(file_id).unwrap();
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("CONFIG_DECLARATION")),
        "config declarations should lower as real HIR, not opaque diagnostics: {diagnostics:?}"
    );
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
