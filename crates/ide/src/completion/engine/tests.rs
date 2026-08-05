use std::path::Path;

use base_db::{change::Change, source_root::SourceRoot};
use utils::text_edit::TextSize;
use vfs::{ChangedFile, FileId, FileSet, VfsPath};

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

    let file_id = FileId::from_raw(0);
    let path = VfsPath::new_virtual_path(path.to_string());

    let mut file_set = FileSet::default();
    file_set.insert(file_id, path);
    let root = SourceRoot::new_local(file_set);

    let mut change = Change::new();
    change.set_roots(vec![root]);
    change.add_changed_file(ChangedFile::create(file_id, owned.as_str()));

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
fn completes_hierarchical_module_root_member_access() {
    let text = r#"
module leaf;
  wire leaf_wire;
endmodule

module top;
  leaf u0();
  initial begin
    top.u0./*caret*/
  end
endmodule
"#;

    let items = completions_in_text(text, Some(TriggerChar::Dot));
    assert!(
        labels(&items).contains(&"leaf_wire"),
        "hierarchical member completion should include child module members: {items:?}"
    );
}

#[test]
fn completion_fixtures() {
    insta::glob!("fixtures/*.v", |path| {
        let fixture = load_fixture(path);
        let items = completions_in_path(&fixture.source, &fixture.path, fixture.trigger);
        insta::assert_debug_snapshot!(items);
    });
}

// ===========================================================================
// TEMPORARY measurement harness — counts salsa query executions behind
// completion / signature help and times representative requests.
//
// Run with:
//   cargo test -p ide --release -- --ignored --nocapture completion_measure
//
// Not a fixture; safe to delete after the investigation.
// ===========================================================================

mod measure {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use preproc_expand::db::PreprocDb;
    use rustc_hash::FxHashMap;
    use tracing::Subscriber;

    use super::*;
    use crate::{
        FilePosition,
        signature_help::{SignatureHelpConfig, signature_help},
    };

    #[derive(Default)]
    struct CountingSubscriber {
        counts: Arc<Mutex<FxHashMap<&'static str, usize>>>,
    }

    impl Subscriber for CountingSubscriber {
        fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            *self.counts.lock().unwrap().entry(attrs.metadata().name()).or_default() += 1;
            tracing::span::Id::from_u64(1)
        }

        fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

        fn event(&self, _event: &tracing::Event<'_>) {}

        fn enter(&self, _span: &tracing::span::Id) {}

        fn exit(&self, _span: &tracing::span::Id) {}
    }

    fn with_counting<T>(f: impl FnOnce() -> T) -> (T, FxHashMap<&'static str, usize>) {
        let subscriber = CountingSubscriber::default();
        let read = Arc::clone(&subscriber.counts);
        let value = tracing::subscriber::with_default(subscriber, f);
        (value, read.lock().unwrap().clone())
    }

    fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
        let start = Instant::now();
        let value = f();
        (value, start.elapsed())
    }

    fn ms(d: Duration) -> String {
        format!("{:.2}", d.as_secs_f64() * 1000.0)
    }

    fn host_with_file(text: &str) -> (AnalysisHost, FileId) {
        let text = normalize_fixture_text(text);
        let file_id = FileId::from_raw(0);
        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/measure.sv".to_owned()));
        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.add_changed_file(ChangedFile::create(file_id, text.as_str()));
        let mut host = AnalysisHost::default();
        host.apply_change(change);
        (host, file_id)
    }

    fn report_counts(label: &str, counts: &FxHashMap<&'static str, usize>) {
        let auth = counts.get("slang.parse_for_compilation").copied().unwrap_or(0);
        let expected = counts.get("slang.parser_expected_syntax").copied().unwrap_or(0);
        let other = counts
            .iter()
            .filter(|(name, _)| {
                **name != "slang.parse_for_compilation" && **name != "slang.parser_expected_syntax"
            })
            .map(|(name, count)| format!("{name} x{count}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "{label:<52} parse_for_compilation x{auth}  parser_expected_syntax x{expected}  {other}"
        );
    }

    /// Statement-position text used for keystroke simulation: caret sits at the
    /// end of the typed prefix inside `initial begin`.
    fn statement_text(prefix: &str) -> String {
        format!("module m;\n  initial begin\n    {prefix}\n  end\nendmodule\n")
    }

    #[test]
    #[ignore]
    fn completion_measure_keystroke_parse_counts() {
        println!("\n== M1: per-keystroke completion in statement context ==");
        let (mut host, file_id) = host_with_file(&statement_text(""));

        let mut cumulative = FxHashMap::default();
        for prefix in ["a", "as", "ass"] {
            let text = statement_text(prefix);
            let mut change = Change::new();
            change.add_changed_file(ChangedFile::create(file_id, text.as_str()));
            host.apply_change(change);

            let offset = TextSize::from(u32::try_from(text.len()).unwrap());
            let position = FilePosition { file_id, offset };

            let ((items, counts), duration) =
                timed(|| with_counting(|| super::completions(host.raw_db(), position, None)));
            for (name, count) in &counts {
                *cumulative.entry(*name).or_insert(0) += count;
            }
            println!("keystroke {prefix:<4} -> {} items, {:<8} ms", items.len(), ms(duration));
            report_counts("  this keystroke", &counts);
        }
        report_counts("cumulative (3 keystrokes)", &cumulative);
    }

    #[test]
    #[ignore]
    fn completion_measure_offset_memoization_and_comment() {
        println!("\n== M2: offset memoization + comment waste ==");
        let (host, file_id) = host_with_file(&statement_text("ass"));
        let offset = TextSize::from(u32::try_from(statement_text("ass").len()).unwrap());
        let position = FilePosition { file_id, offset };

        let (_, counts_first) = with_counting(|| super::completions(host.raw_db(), position, None));
        report_counts("first request (fresh offset)", &counts_first);

        let (_, counts_again) = with_counting(|| super::completions(host.raw_db(), position, None));
        report_counts("same request again (memoized offset)", &counts_again);

        // A *different* offset in the same file, same db: expectation parse reruns.
        let offset2 = offset - TextSize::new(1);
        let position2 = FilePosition { file_id, offset: offset2 };
        let (_, counts_fresh) =
            with_counting(|| super::completions(host.raw_db(), position2, None));
        report_counts("new offset, same file version", &counts_fresh);

        // Completion inside a line comment: lex context short-circuits the
        // engine, but does the expectation parse still run?
        let comment_text = "module m;\n  // note comment\nendmodule\n";
        let (host2, file_id2) = host_with_file(comment_text);
        let caret = comment_text.find("comment").unwrap() + 3;
        let position3 = FilePosition { file_id: file_id2, offset: TextSize::from(caret as u32) };
        let ((items, counts_comment), duration) =
            timed(|| with_counting(|| super::completions(host2.raw_db(), position3, None)));
        println!("comment completion -> {} items, {:<8} ms", items.len(), ms(duration));
        report_counts("comment completion request", &counts_comment);
    }

    fn module_text(name: u32) -> String {
        format!(
            "module m{name}(input logic clk);\n  logic a{name}, b{name};\n  assign a{name} = b{name} ^ clk;\n  always_ff @(posedge clk) b{name} <= a{name};\nendmodule\n"
        )
    }

    fn scale_text(modules: u32) -> String {
        let mut text = String::new();
        for n in 0..modules {
            text.push_str(&module_text(n));
        }
        text.push_str("module top;\n  initial begin\n    w\n  end\nendmodule\n");
        text
    }

    #[test]
    #[ignore]
    fn completion_measure_latency_scale() {
        println!("\n== M3: latency vs file size (fresh offset vs warm) ==");
        println!(
            "{:<10} {:<10} {:<14} {:<14} {:<14}",
            "modules", "bytes", "cold", "warm same", "fresh offset"
        );
        for modules in [64u32, 256, 1024] {
            let text = scale_text(modules);
            let (host, file_id) = host_with_file(&text);
            let end = text.len();
            let pos = |offset: usize| FilePosition {
                file_id,
                offset: TextSize::from(u32::try_from(offset).unwrap()),
            };

            let (_, cold) =
                timed(|| std::hint::black_box(super::completions(host.raw_db(), pos(end), None)));
            let (_, warm) =
                timed(|| std::hint::black_box(super::completions(host.raw_db(), pos(end), None)));
            let (_, fresh) = timed(|| {
                std::hint::black_box(super::completions(host.raw_db(), pos(end - 1), None))
            });
            println!(
                "{modules:<10} {:<10} {:<14} {:<14} {:<14}",
                text.len(),
                ms(cold),
                ms(warm),
                ms(fresh)
            );
        }
    }

    #[test]
    #[ignore]
    fn completion_measure_expected_syntax_volume() {
        println!("\n== M5: recordAll volume + in-tree query cost ==");
        println!("{:<10} {:<10} {:<16} {:<14}", "modules", "bytes", "found_at_eof", "query_ms");
        for modules in [64u32, 256, 1024] {
            let text = scale_text(modules);
            let (host, file_id) = host_with_file(&text);
            let db = host.raw_db();
            let tree = db.parsed_compilation_unit(file_id).syntax_tree;
            let end = text.len();
            let (found, query) = timed(|| tree.expected_syntax_at(end));
            println!("{modules:<10} {:<10} {:<16} {:<14}", text.len(), found.len(), ms(query));
        }
    }

    #[test]
    #[ignore]
    fn signature_help_measure_parse_counts() {
        println!("\n== M4: signature help parse counts ==");
        let text = "module child #(parameter W = 8) (input logic [W-1:0] data, output logic [W-1:0] out);\nendmodule\n\nmodule top;\n  child u0(/*caret*/data, out);\nendmodule\n";
        let (host, file_id) = host_with_file(text);
        let caret = text.find("/*caret*/").unwrap();
        let position = FilePosition { file_id, offset: TextSize::from(caret as u32) };
        let ((result, counts), duration) = timed(|| {
            with_counting(|| {
                signature_help(host.raw_db(), position, SignatureHelpConfig { params_only: false })
            })
        });
        println!("port-list signature help -> {:?}, {:<8} ms", result.is_some(), ms(duration));
        report_counts("signature help request", &counts);

        // Param list `#(...)` form.
        let text2 = "module child #(parameter W = 8) (input logic [W-1:0] data);\nendmodule\n\nmodule top;\n  child #(/*caret*/8) u0();\nendmodule\n";
        let (host2, file_id2) = host_with_file(text2);
        let caret2 = text2.find("/*caret*/").unwrap();
        let position2 = FilePosition { file_id: file_id2, offset: TextSize::from(caret2 as u32) };
        let ((result2, counts2), duration2) = timed(|| {
            with_counting(|| {
                signature_help(
                    host2.raw_db(),
                    position2,
                    SignatureHelpConfig { params_only: false },
                )
            })
        });
        println!("param-list signature help -> {:?}, {:<8} ms", result2.is_some(), ms(duration2));
        report_counts("param-list signature help request", &counts2);
    }
}
