use std::path::Path;

use hir::base_db::{change::Change, source_root::SourceRoot};
use triomphe::Arc;
use utils::{lines::LineEnding, text_edit::TextSize};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use super::*;
use crate::{
    analysis_host::AnalysisHost, completion::context::TriggerChar,
    test_utils::normalize_fixture_text,
};

fn setup(text: &str) -> (AnalysisHost, FilePosition) {
    setup_with_path(text, "/test.v")
}

fn setup_with_path(text: &str, path: &str) -> (AnalysisHost, FilePosition) {
    let text = normalize_fixture_text(text);
    let marker = "/*caret*/";
    let off = text.find(marker).expect("missing /*caret*/");
    let mut owned = text;
    owned = owned.replace(marker, "");

    let file_id = FileId(0);
    let path = VfsPath::new_virtual_path(path.to_string());

    let mut file_set = FileSet::default();
    file_set.insert(file_id, path);
    let root = SourceRoot::new_local(file_set);

    let mut change = Change::new();
    change.set_roots(vec![root]);
    change.add_changed_file(ChangedFile {
        file_id,
        change_kind: ChangeKind::Create(Arc::from(owned.as_str()), LineEnding::Unix),
    });

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    let position = FilePosition { file_id, offset: TextSize::from(off as u32) };
    (host, position)
}

fn completions_in_text(text: &str, trigger: Option<TriggerChar>) -> Vec<CompletionItem> {
    let (host, position) = setup(text);
    super::completions(host.raw_db(), position, trigger)
}

fn completions_in_library_map(text: &str, trigger: Option<TriggerChar>) -> Vec<CompletionItem> {
    let (host, position) = setup_with_path(text, "/test.map");
    super::completions(host.raw_db(), position, trigger)
}

fn labels(items: &[CompletionItem]) -> Vec<&str> {
    items.iter().map(|item| item.label.as_str()).collect()
}

fn parse_trigger(line: &str) -> Option<TriggerChar> {
    let line = line.trim();
    let prefix = "// trigger:";
    if !line.starts_with(prefix) {
        return None;
    }

    match line[prefix.len()..].trim() {
        "." => Some(TriggerChar::Dot),
        "(" => Some(TriggerChar::OpenParen),
        "," => Some(TriggerChar::Comma),
        "@" => Some(TriggerChar::At),
        "#" => Some(TriggerChar::Hash),
        "$" => Some(TriggerChar::Dollar),
        "`" => Some(TriggerChar::Backtick),
        "'" => Some(TriggerChar::Apostrophe),
        "\\n" => Some(TriggerChar::Newline),
        _ => None,
    }
}

fn load_fixture(path: &Path) -> (String, Option<TriggerChar>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
    let text = normalize_fixture_text(&text);
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return (text, None);
    };

    if let Some(trigger) = parse_trigger(first) {
        let remaining = lines.collect::<Vec<_>>().join("\n");
        return (remaining, Some(trigger));
    }

    (text, None)
}

#[test]
fn module_item_identifier_prefix_stays_module_item_start() {
    let items =
        completions_in_text("module Foo; endmodule\nmodule top; Fo/*caret*/\nendmodule\n", None);
    let labels = labels(&items);

    assert!(labels.contains(&"Foo"), "module instantiation snippet expected: {items:?}");
    assert!(
        !labels.contains(&"top"),
        "module item prefix should not be treated as expression: {items:?}"
    );
}

#[test]
fn generate_item_completion_includes_module_instantiation_snippets() {
    let items = completions_in_text(
        "module Foo; endmodule\nmodule top; generate\n  Fo/*caret*/\nendgenerate endmodule\n",
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"Foo"), "generate module instantiation expected: {items:?}");
}

#[test]
fn top_level_completion_keeps_top_level_keyword_prefixes() {
    for (text, expected) in [("con/*caret*/\n", "config"), ("pri/*caret*/\n", "primitive")] {
        let items = completions_in_text(text, None);
        assert!(labels(&items).contains(&expected), "{expected} missing from {items:?}");
    }
}

#[test]
fn prefix_module_completion_at_file_start_does_not_panic() {
    let items = completions_in_text("/*caret*/module counter(input clk", None);
    let item_labels = labels(&items);

    assert!(item_labels.contains(&"module"), "top-level module keyword expected: {items:?}");
}

#[test]
fn library_map_completion_uses_library_map_keywords() {
    let items = completions_in_library_map("lib/*caret*/\n", None);
    let item_labels = labels(&items);

    assert!(item_labels.contains(&"library"), "library map keyword expected: {items:?}");
    assert!(!item_labels.contains(&"module"), "SV keyword leaked into library map: {items:?}");
}

#[test]
fn module_member_completion_excludes_procedural_statement_snippets() {
    let items = completions_in_text("module m;\n  /*caret*/\nendmodule\n", None);
    let labels = labels(&items);

    assert!(labels.contains(&"always"), "module procedural block expected: {items:?}");
    assert!(labels.contains(&"begin"), "generate block expected: {items:?}");
    assert!(labels.contains(&"wire"), "module declarations expected: {items:?}");
    assert!(!labels.contains(&"while"), "statement snippet leaked into module member: {items:?}");
    assert!(!labels.contains(&"return"), "jump statement leaked into module member: {items:?}");
}

#[test]
fn generate_completion_uses_generate_member_keywords() {
    let items =
        completions_in_text("module m; generate\n  wi/*caret*/\nendgenerate endmodule\n", None);
    let item_labels = labels(&items);

    assert!(item_labels.contains(&"wire"), "generate declaration expected: {items:?}");
    assert!(!item_labels.contains(&"while"), "statement leaked into generate item: {items:?}");

    let items =
        completions_in_text("module m; generate\n  as/*caret*/\nendgenerate endmodule\n", None);
    let item_labels = labels(&items);
    assert!(item_labels.contains(&"assign"), "generate continuous assign expected: {items:?}");
}

#[test]
fn specify_completion_uses_specify_item_keywords() {
    let items =
        completions_in_text("module m; specify\n  sp/*caret*/\nendspecify endmodule\n", None);
    let labels = labels(&items);

    assert!(labels.contains(&"specparam"), "specify declaration expected: {items:?}");
    assert!(
        !labels.contains(&"specify"),
        "module specify block leaked into specify item: {items:?}"
    );
}

#[test]
fn config_completion_uses_config_phase_keywords() {
    let items =
        completions_in_text("config cfg;\n  de/*caret*/\n  design work.top;\nendconfig\n", None);
    let header_labels = labels(&items);
    assert!(header_labels.contains(&"design"), "config design keyword expected: {items:?}");
    assert!(
        !header_labels.contains(&"default"),
        "config rule keyword leaked before design clause: {items:?}"
    );

    let items =
        completions_in_text("config cfg;\n  design work.top;\n  de/*caret*/\nendconfig\n", None);
    let rule_labels = labels(&items);
    assert!(rule_labels.contains(&"default"), "config rule keyword expected: {items:?}");
    assert!(!rule_labels.contains(&"design"), "config design keyword leaked into rules: {items:?}");
}

#[test]
fn module_member_completion_includes_gate_primitives() {
    let items = completions_in_text("module m;\n  bu/*caret*/\nendmodule\n", None);
    let item_labels = labels(&items);
    assert!(item_labels.contains(&"buf"), "buf primitive expected: {items:?}");
    assert!(item_labels.contains(&"bufif0"), "bufif0 primitive expected: {items:?}");
    assert!(item_labels.contains(&"bufif1"), "bufif1 primitive expected: {items:?}");

    let items = completions_in_text("module m;\n  a/*caret*/\nendmodule\n", None);
    let item_labels = labels(&items);
    assert!(item_labels.contains(&"and"), "and primitive expected: {items:?}");

    let items = completions_in_text("module m;\n  as/*caret*/\nendmodule\n", None);
    let item_labels = labels(&items);
    assert!(item_labels.contains(&"assign"), "continuous assign expected: {items:?}");
}

#[test]
fn module_member_completion_includes_parameter_declaration_snippets() {
    let items = completions_in_text("module m;\n  lo/*caret*/\nendmodule\n", None);
    let item_labels = labels(&items);

    assert!(item_labels.contains(&"localparam"), "localparam keyword expected: {items:?}");
}

#[test]
fn parameter_port_keyword_prefix_completes_before_decl_name() {
    let items = completions_in_text("module m #(para/*caret*/) (); endmodule\n", None);
    let item_labels = labels(&items);

    assert!(item_labels.contains(&"parameter"), "parameter keyword expected: {items:?}");
}

#[test]
fn ansi_port_keyword_prefix_completes_before_decl_name() {
    let items = completions_in_text("module m(input wir/*caret*/); endmodule\n", None);
    let item_labels = labels(&items);

    assert!(item_labels.contains(&"wire"), "wire keyword expected: {items:?}");
}

#[test]
fn gate_primitives_do_not_leak_into_statement_completion() {
    let items = completions_in_text("module m; initial begin\n  a/*caret*/\nend endmodule\n", None);
    let item_labels = labels(&items);

    assert!(!item_labels.contains(&"and"), "gate primitive leaked into statement list: {items:?}");
}

#[test]
fn block_decl_completion_allows_decls_and_statements() {
    let items = completions_in_text("module m; initial begin\n  /*caret*/\nend endmodule\n", None);
    let labels = labels(&items);

    assert!(labels.contains(&"integer"), "block declaration expected: {items:?}");
    assert!(labels.contains(&"if"), "statement keyword expected: {items:?}");
    assert!(!labels.contains(&"wire"), "net declaration leaked into procedural block: {items:?}");
    assert!(!labels.contains(&"always"), "module item leaked into procedural block: {items:?}");
}

#[test]
fn block_decl_prefix_completion_allows_decls_and_statements() {
    let items =
        completions_in_text("module m; initial begin\n  re/*caret*/\nend endmodule\n", None);
    let labels = labels(&items);

    assert!(labels.contains(&"reg"), "block declaration prefix expected: {items:?}");
    assert!(labels.contains(&"repeat"), "statement prefix expected: {items:?}");
}

#[test]
fn statement_completion_after_statement_excludes_decls_and_module_items() {
    let items = completions_in_text(
        "module m; initial begin\n  x = 1;\n  /*caret*/\nend endmodule\n",
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"if"), "statement keyword expected: {items:?}");
    assert!(!labels.contains(&"integer"), "declaration leaked after statement: {items:?}");
    assert!(!labels.contains(&"always"), "module item leaked into statement list: {items:?}");
}

#[test]
fn statement_completion_after_if_block_includes_else_snippets() {
    let items = completions_in_text(
        r#"
module m;
  initial begin
    if (cond) begin
    end
    el/*caret*/
  end
endmodule
"#,
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"else"), "else block snippet expected: {items:?}");
    assert!(labels.contains(&"else if"), "else-if block snippet expected: {items:?}");

    let else_item = items.iter().find(|item| item.label == "else").unwrap();
    assert_eq!(else_item.snippet_edit.as_ref().unwrap().ins, "else begin\n\t${0}\nend");

    let else_if_item = items.iter().find(|item| item.label == "else if").unwrap();
    assert_eq!(
        else_if_item.snippet_edit.as_ref().unwrap().ins,
        "else if (${1:cond}) begin\n\t${0}\nend"
    );
}

#[test]
fn no_completion_in_block_decl_name_gap() {
    let items =
        completions_in_text("module m; initial begin\n  integer /*caret*/;\nend endmodule\n", None);
    assert!(items.is_empty(), "declaration name gap should not complete keywords: {items:?}");
}

#[test]
fn incomplete_member_access_uses_structural_left_expression() {
    let items = completions_in_text(
        "module sub; wire inner; endmodule\nmodule top; sub u0(); initial u0./*caret*/ endmodule\n",
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"inner"), "member access should recover left expression: {items:?}");
}

#[test]
fn incomplete_chained_member_access_uses_structural_left_expression() {
    let items = completions_in_text(
        "module leaf; wire leaf_wire; endmodule\nmodule sub; leaf u1(); endmodule\nmodule top; sub u0(); initial u0.u1./*caret*/ endmodule\n",
        None,
    );
    let labels = labels(&items);

    assert!(
        labels.contains(&"leaf_wire"),
        "chained member access should recover left expression: {items:?}"
    );
}

#[test]
fn incomplete_array_member_access_uses_structural_left_expression() {
    let items = completions_in_text(
        "module sub; wire inner; endmodule\nmodule top; sub u0 [0:1] (); initial u0[0]./*caret*/ endmodule\n",
        None,
    );
    let labels = labels(&items);

    assert!(
        labels.contains(&"inner"),
        "array member access should recover left expression: {items:?}"
    );
}

#[test]
fn scoped_name_completion_uses_package_scope() {
    let items = completions_in_text(
        r#"
package pkg;
  localparam int pkg_value = 1;
  localparam int other_value = 2;
endpackage

module top;
  localparam int value = pkg::/*caret*/;
endmodule
"#,
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"pkg_value"), "package member expected: {items:?}");
    assert!(labels.contains(&"other_value"), "package member expected: {items:?}");
}

#[test]
fn scoped_name_completion_filters_prefix() {
    let items = completions_in_text(
        r#"
package pkg;
  localparam int pkg_value = 1;
  localparam int other_value = 2;
endpackage

module top;
  localparam int value = pkg::pkg/*caret*/;
endmodule
"#,
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"pkg_value"), "prefixed package member expected: {items:?}");
    assert!(!labels.contains(&"other_value"), "non-matching package member leaked: {items:?}");
}

#[test]
fn member_access_completion_uses_struct_fields() {
    let items = completions_in_text(
        r#"
module top;
  typedef struct {
    logic [7:0] first_field;
    logic [7:0] second_field;
  } packet_t;
  packet_t pkt;
  initial pkt./*caret*/
endmodule
"#,
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"first_field"), "struct field expected: {items:?}");
    assert!(labels.contains(&"second_field"), "struct field expected: {items:?}");
}

#[test]
fn manual_and_triggered_at_use_same_sensitivity_expectation_behavior() {
    let text = "module m; wire clk; always @/*caret*/(posedge clk) begin end endmodule\n";
    let manual = completions_in_text(text, None);
    let triggered = completions_in_text(text, Some(TriggerChar::At));

    assert_eq!(manual, triggered);
    assert!(labels(&manual).contains(&"*"), "sensitivity completions expected: {manual:?}");
}

#[test]
fn unresolved_instantiation_does_not_complete_connections() {
    let items = completions_in_text("module top; missing u0(/*caret*/); endmodule\n", None);
    assert!(items.is_empty(), "unresolved instantiation should not fall back: {items:?}");
}

#[test]
fn named_port_expr_without_known_type_does_not_fallback_to_all_values() {
    let items = completions_in_text(
        "module m(input custom_t a); endmodule\nmodule top; wire sig; m u0(.a(/*caret*/)); endmodule\n",
        None,
    );
    assert!(items.is_empty(), "unknown typed port should not accept all values: {items:?}");
}

#[test]
fn completion_fixtures() {
    insta::glob!("fixtures/*.v", |path| {
        let (text, trigger) = load_fixture(&path);
        let items = completions_in_text(&text, trigger);
        insta::assert_debug_snapshot!(items);
    });
}
