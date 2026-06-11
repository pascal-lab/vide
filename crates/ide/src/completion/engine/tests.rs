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
    completions_in_path(text, "/test.v", trigger)
}

fn completions_in_path(
    text: &str,
    path: &str,
    trigger: Option<TriggerChar>,
) -> Vec<CompletionItem> {
    let (host, position) = setup_with_path(text, path);
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

struct CompletionFixture {
    source: String,
    path: String,
    trigger: Option<TriggerChar>,
}

fn parse_fixture_path(line: &str) -> Option<String> {
    let line = line.trim();
    let prefix = "// path:";
    line.starts_with(prefix).then(|| line[prefix.len()..].trim().to_string())
}

fn load_fixture(path: &Path) -> CompletionFixture {
    let text = std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
    let text = normalize_fixture_text(&text);
    let mut offset = 0;
    let mut fixture_path = "/test.v".to_string();
    let mut trigger = None;

    while offset < text.len() {
        let rest = &text[offset..];
        let line_len = rest.find('\n').map_or(rest.len(), |idx| idx + 1);
        let line_with_newline = &rest[..line_len];
        let line = line_with_newline.strip_suffix('\n').unwrap_or(line_with_newline);

        if let Some(value) = parse_trigger(line) {
            trigger = Some(value);
            offset += line_len;
            continue;
        }

        if let Some(value) = parse_fixture_path(line) {
            fixture_path = value;
            offset += line_len;
            continue;
        }

        break;
    }

    CompletionFixture { source: text[offset..].to_string(), path: fixture_path, trigger }
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
fn completion_fixtures() {
    insta::glob!("fixtures/*.v", |path| {
        let fixture = load_fixture(path);
        let items = completions_in_path(&fixture.source, &fixture.path, fixture.trigger);
        insta::assert_debug_snapshot!(items);
    });
}
