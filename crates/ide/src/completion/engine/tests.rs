use std::path::PathBuf;

use base_db::{change::Change, source_root::SourceRoot};
use insta::assert_debug_snapshot;
use triomphe::Arc;
use utils::{lines::LineEnding, text_edit::TextSize};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use super::*;
use crate::{
    analysis_host::AnalysisHost, completion::context::TriggerChar,
    test_utils::normalize_fixture_text,
};

fn setup(text: &str) -> (AnalysisHost, FilePosition) {
    let text = normalize_fixture_text(text);
    let marker = "/*caret*/";
    let off = text.find(marker).expect("missing /*caret*/");
    let mut owned = text;
    owned = owned.replace(marker, "");

    let file_id = FileId(0);
    let path = VfsPath::new_virtual_path("/test.v".to_string());

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

fn labels(items: &[CompletionItem]) -> Vec<&str> {
    items.iter().map(|item| item.label.as_str()).collect()
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/completion/engine/fixtures")
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
        "`" => Some(TriggerChar::Backtick),
        "\\n" => Some(TriggerChar::Newline),
        _ => None,
    }
}

fn load_fixture(path: &PathBuf) -> (String, Option<TriggerChar>) {
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
fn no_completion_in_line_comment_at_eof_top_level() {
    let items = completions_in_text("// ,/*caret*/", None);
    assert!(items.is_empty());
}

#[test]
fn no_completion_in_line_comment_at_file_start() {
    // regression: line comment before any module should not trigger completion
    let items = completions_in_text("// hello /*caret*/world\nmodule m; endmodule\n", None);
    assert!(items.is_empty());
}

#[test]
fn no_completion_in_line_comment_with_comma_trigger() {
    // regression: comma trigger in line comment should not trigger completion
    let items =
        completions_in_text("// ,,/*caret*/\nmodule m; endmodule\n", Some(TriggerChar::Comma));
    assert!(items.is_empty());
}

#[test]
fn no_completion_in_line_comment_middle_of_file() {
    // regression: line comment between modules should not trigger completion
    let items = completions_in_text(
        "// first line\n// second line ,/*caret*/\n\nmodule m; endmodule\n",
        Some(TriggerChar::Comma),
    );
    assert!(items.is_empty());
}

#[test]
fn no_completion_in_line_comment_user_reported_case() {
    // exact reproduction of user's file with // ,, at line 6
    let text = r#"// when declaring new symbol, after typing the type, the completion should not suggest anything

// when in trivia and string literals, no completion should be suggested

// those keywords complete in modules (input, etc) should also be suggested in tasks and functions

// ,,/*caret*/

`timescale 1ns / 1ps

module adder (
    input  [3:0] a,
    input  [3:0] b,
    output [4:0] y
);
endmodule
"#;
    let items = completions_in_text(text, Some(TriggerChar::Comma));
    assert!(items.is_empty(), "should not complete in line comment, got: {:?}", items);
}

#[test]
fn no_completion_in_line_comment_before_timescale() {
    // simpler reproduction: comment line right before `timescale directive
    let text = r#"// comment ,/*caret*/
`timescale 1ns / 1ps
module m; endmodule
"#;
    let items = completions_in_text(text, Some(TriggerChar::Comma));
    assert!(
        items.is_empty(),
        "should not complete in line comment before `timescale, got: {:?}",
        items
    );
}

#[test]
fn no_completion_in_line_comment_before_module() {
    // comment line right before module (no directive)
    let text = r#"// comment ,/*caret*/
module m; endmodule
"#;
    let items = completions_in_text(text, Some(TriggerChar::Comma));
    assert!(
        items.is_empty(),
        "should not complete in line comment before module, got: {:?}",
        items
    );
}

#[test]
fn no_completion_inside_literal() {
    let items = completions_in_text("module m; initial x = 12/*caret*/34; endmodule\n", None);
    assert!(items.is_empty(), "should not complete in numeric literal, got: {:?}", items);
}

#[test]
fn no_completion_inside_based_literal() {
    let items = completions_in_text("module m; initial x = 4'b10/*caret*/10; endmodule\n", None);
    assert!(items.is_empty(), "should not complete in based numeric literal, got: {:?}", items);
}

#[test]
fn no_completion_while_typing_based_literal() {
    for text in [
        "module m; initial x = 4'/*caret*/; endmodule\n",
        "module m; initial x = 4'b/*caret*/; endmodule\n",
        "module m; initial x = 4'b0001/*caret*/; endmodule\n",
    ] {
        let items = completions_in_text(text, None);
        assert!(
            items.is_empty(),
            "should not complete while typing based literal, got: {:?}",
            items
        );
    }
}

#[test]
fn no_completion_at_top_level_with_comma_trigger() {
    let items = completions_in_text(",/*caret*/\nmodule m; endmodule\n", Some(TriggerChar::Comma));
    assert!(
        items.is_empty(),
        "should not complete at top level on comma trigger, got: {:?}",
        items
    );
}

#[test]
fn preproc_completion_preserves_typed_backtick() {
    let items = completions_in_text("module m; initial `de/*caret*/; endmodule\n", None);
    let define = items
        .iter()
        .find(|item| item.label == "define" && item.kind == CompletionItemKind::Keyword)
        .expect("define directive completion expected");
    let mut text = "module m; initial `de; endmodule\n".to_string();
    define.edit.as_ref().unwrap().apply_on(&mut text);

    assert_eq!(text, "module m; initial `define; endmodule\n");
}

#[test]
fn no_directive_keyword_completion_in_directive_body() {
    let items = completions_in_text("`define /*caret*/FOO 1\nmodule m; endmodule\n", None);
    assert!(items.is_empty(), "directive body should not suggest directive keywords: {items:?}");
}

#[test]
fn expression_completion_excludes_module_item_keywords() {
    let items = completions_in_text(
        "module m; wire a; wire out; assign out = a + /*caret*/; endmodule\n",
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"a"), "expression names should be offered: {items:?}");
    assert!(!labels.contains(&"assign"), "module item keyword leaked into expression: {items:?}");
    assert!(!labels.contains(&"always"), "module item snippet leaked into expression: {items:?}");
}

#[test]
fn initializer_expression_completion_excludes_module_item_keywords() {
    let items =
        completions_in_text("module m; wire a; localparam P = a + /*caret*/; endmodule\n", None);
    let labels = labels(&items);

    assert!(labels.contains(&"a"), "initializer expression names should be offered: {items:?}");
    assert!(!labels.contains(&"assign"), "module item keyword leaked into initializer: {items:?}");
}

#[test]
fn subroutine_completion_resolves_named_generate_block_container() {
    let items = completions_in_text(
        r#"
module top;
  generate
    if (1) begin : g
      function integer f;
        input integer arg;
        integer local_value;
        begin
          local_value = arg + /*caret*/local_value;
        end
      endfunction
    end
  endgenerate
endmodule
"#,
        None,
    );
    let labels = labels(&items);

    assert!(labels.contains(&"arg"), "subroutine port expected: {items:?}");
    assert!(labels.contains(&"local_value"), "subroutine local expected: {items:?}");
}

#[test]
fn subroutine_completion_resolves_single_member_generate_container() {
    let text = r#"
module top;
  generate
    if (1)
      function integer f;
        input integer arg;
        integer local_value;
        begin
          local_value = arg + /*caret*/local_value;
        end
      endfunction
  endgenerate
endmodule
"#;
    let items = completions_in_text(text, None);
    let labels = labels(&items);

    assert!(labels.contains(&"arg"), "subroutine port expected: {items:?}");
    assert!(labels.contains(&"local_value"), "subroutine local expected: {items:?}");
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
fn top_level_completion_keeps_top_level_keyword_prefixes() {
    for (text, expected) in [
        ("con/*caret*/\n", "config"),
        ("pri/*caret*/\n", "primitive"),
        ("lib/*caret*/\n", "library"),
        ("lib/*caret*/\n", "liblist"),
    ] {
        let items = completions_in_text(text, None);
        assert!(labels(&items).contains(&expected), "{expected} missing from {items:?}");
    }
}

#[test]
fn module_member_completion_excludes_procedural_statement_snippets() {
    let items = completions_in_text("module m;\n  /*caret*/\nendmodule\n", None);
    let labels = labels(&items);

    assert!(labels.contains(&"always"), "module procedural block expected: {items:?}");
    assert!(labels.contains(&"wire"), "module declarations expected: {items:?}");
    assert!(!labels.contains(&"while"), "statement snippet leaked into module member: {items:?}");
    assert!(!labels.contains(&"return"), "jump statement leaked into module member: {items:?}");
    assert!(!labels.contains(&"begin"), "block statement leaked into module member: {items:?}");
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
    let dir = fixtures_dir();
    let mut fixtures: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("failed to read fixtures dir {dir:?}: {err}"))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? != "v" {
                return None;
            }
            let name = path.file_stem()?.to_string_lossy().to_string();
            Some((name, path))
        })
        .collect();

    fixtures.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!fixtures.is_empty(), "no fixtures found in {dir:?}");

    for (name, path) in fixtures {
        let (text, trigger) = load_fixture(&path);
        let items = completions_in_text(&text, trigger);
        assert_debug_snapshot!(name, items);
    }
}
