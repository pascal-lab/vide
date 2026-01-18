use std::path::PathBuf;

use base_db::{change::Change, source_root::SourceRoot};
use insta::assert_debug_snapshot;
use triomphe::Arc;
use utils::{lines::LineEnding, text_edit::TextSize};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use super::*;
use crate::{analysis_host::AnalysisHost, completion::context::TriggerChar};

fn setup(text: &str) -> (AnalysisHost, FilePosition) {
    let marker = "/*caret*/";
    let off = text.find(marker).expect("missing /*caret*/");
    let mut owned = text.to_string();
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
        _ => None,
    }
}

fn load_fixture(path: &PathBuf) -> (String, Option<TriggerChar>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
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
fn no_completion_at_top_level_with_comma_trigger() {
    let items = completions_in_text(",/*caret*/\nmodule m; endmodule\n", Some(TriggerChar::Comma));
    assert!(
        items.is_empty(),
        "should not complete at top level on comma trigger, got: {:?}",
        items
    );
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
