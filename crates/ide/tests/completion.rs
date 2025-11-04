use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use base_db::source_db::SourceDb;
use ide::{
    analysis::Analysis,
    analysis_host::AnalysisHost,
    completion::{CompletionConfig, CompletionItem, CompletionItemKind},
};
use rustc_hash::FxHashSet;
use serde::Deserialize;
use span::FilePosition;
use utils::text_edit::TextSize;
use vfs::FileId;

#[derive(Debug, Deserialize)]
struct Manifest {
    cases: Vec<TestCase>,
}

#[derive(Debug, Deserialize)]
struct TestCase {
    name: String,
    fixture: String,
    #[serde(default)]
    caret_replace: Option<String>,
    #[serde(default)]
    config: CaseConfig,
    #[serde(default)]
    trigger: Option<String>,
    #[serde(default)]
    expect: Expectation,
}

#[derive(Debug, Default, Deserialize)]
struct CaseConfig {
    #[serde(default)]
    enable_snippets: bool,
}

#[derive(Debug, Default, Deserialize)]
struct Expectation {
    #[serde(default)]
    present: Vec<ExpectedItem>,
    #[serde(default)]
    absent: Vec<String>,
    #[serde(default)]
    min_items: Option<usize>,
    #[serde(default)]
    max_items: Option<usize>,
    #[serde(default)]
    min_keyword_items: Option<usize>,
    #[serde(default)]
    order_prefix: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ExpectedItem {
    label: String,
    #[serde(default)]
    kind: Option<ItemKind>,
    #[serde(default)]
    detail_contains: Option<String>,
    #[serde(default)]
    insert_text_contains: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ItemKind {
    Keyword,
    Snippet,
    Module,
    Type,
    Function,
    Variable,
    Field,
    Identifier,
    Unknown,
}

impl ItemKind {
    fn to_completion_kind(&self) -> CompletionItemKind {
        match self {
            ItemKind::Keyword => CompletionItemKind::Keyword,
            ItemKind::Snippet => CompletionItemKind::Snippet,
            ItemKind::Module => CompletionItemKind::Module,
            ItemKind::Type => CompletionItemKind::Type,
            ItemKind::Function => CompletionItemKind::Function,
            ItemKind::Variable => CompletionItemKind::Variable,
            ItemKind::Field => CompletionItemKind::Field,
            ItemKind::Identifier => CompletionItemKind::Identifier,
            ItemKind::Unknown => CompletionItemKind::Unknown,
        }
    }
}

#[test]
fn completion_fixtures() {
    let manifest = load_manifest();

    for case in &manifest.cases {
        eprintln!("{:#?} `{}`", case.name, case.fixture);
        run_case(case);
    }
}

fn run_case(case: &TestCase) {
    let (source, offset) = load_fixture_source(case);
    let (analysis, file_id) = create_analysis_with_file(&source, &case.name);

    let trigger = case.trigger.as_ref().map(|s| {
        if s.chars().count() != 1 {
            panic!("case `{}`: trigger `{}` must be a single character", case.name, s);
        }
        s.chars().next().unwrap()
    });

    let position = FilePosition { file_id, offset };
    let config = CompletionConfig { enable_snippets: case.config.enable_snippets };

    let result = analysis
        .completion(position, config, trigger)
        .unwrap_or_else(|_| panic!("case `{}`: completion returned cancellation", case.name));

    assert_expectations(&case.name, &result.items, &case.expect);
}

fn load_fixture_source(case: &TestCase) -> (String, TextSize) {
    let path = fixtures_dir().join(&case.fixture);
    let data = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("case `{}`: failed to read fixture `{}`: {err}", case.name, path.display());
    });

    const MARKER: &str = "$0";
    let marker_index = data.find(MARKER).unwrap_or_else(|| {
        panic!(
            "case `{}`: fixture `{}` does not contain caret marker `$0`",
            case.name,
            path.display()
        )
    });

    if data[marker_index + MARKER.len()..].contains(MARKER) {
        panic!("case `{}`: fixture `{}` contains multiple `$0` markers", case.name, path.display());
    }

    let replacement = case.caret_replace.as_deref().unwrap_or("");

    let mut source = data.clone();
    source.replace_range(marker_index..marker_index + MARKER.len(), replacement);

    let byte_index = u32::try_from(marker_index + replacement.len()).expect("offset overflow");
    let offset = TextSize::from(byte_index);

    (source, offset)
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("completion")
}

fn load_manifest() -> Manifest {
    let manifest_path = fixtures_dir().join("manifest.json");
    let data = fs::read_to_string(&manifest_path).unwrap_or_else(|err| {
        panic!("failed to read manifest `{}`: {err}", manifest_path.display())
    });

    serde_json::from_str(&data).unwrap_or_else(|err| {
        panic!("failed to parse manifest `{}`: {err}", manifest_path.display())
    })
}

fn create_analysis_with_file(content: &str, unique: &str) -> (Analysis, FileId) {
    let mut host = AnalysisHost::default();

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    unique.hash(&mut hasher);
    let unique_id = ((hasher.finish() % 900_000) + 100_000) as u32;
    let file_id = FileId(unique_id);

    host.raw_db_mut().set_file_text(file_id, content.into());

    let mut files = FxHashSet::default();
    files.insert(file_id);
    host.raw_db_mut().set_files(Box::new(files));

    (host.make_analysis(), file_id)
}

fn assert_expectations(case: &str, items: &[CompletionItem], expect: &Expectation) {
    if let Some(min_items) = expect.min_items {
        assert!(
            items.len() >= min_items,
            "case `{}`: expected at least {} items, found {}",
            case,
            min_items,
            items.len()
        );
    }

    if let Some(max_items) = expect.max_items {
        assert!(
            items.len() <= max_items,
            "case `{}`: expected at most {} items, found {}",
            case,
            max_items,
            items.len()
        );
    }

    if let Some(min_keywords) = expect.min_keyword_items {
        let actual = items.iter().filter(|item| item.kind == CompletionItemKind::Keyword).count();
        assert!(
            actual >= min_keywords,
            "case `{}`: expected at least {} keyword items, found {}",
            case,
            min_keywords,
            actual
        );
    }

    for expected in &expect.present {
        let item = items.iter().find(|item| item.label == expected.label).unwrap_or_else(|| {
            panic!(
                "case `{}`: expected item `{}` to be present. Available labels: {:?}",
                case,
                expected.label,
                items.iter().map(|item| &item.label).collect::<Vec<_>>()
            )
        });

        if let Some(kind) = &expected.kind {
            assert_eq!(
                item.kind,
                kind.to_completion_kind(),
                "case `{}`: expected `{}` to have kind {:?}, found {:?}",
                case,
                expected.label,
                kind,
                item.kind
            );
        }

        if let Some(substr) = &expected.detail_contains {
            let detail = item.detail.as_ref().unwrap_or_else(|| {
                panic!(
                    "case `{}`: expected `{}` to have detail containing `{}`, but detail was None",
                    case, expected.label, substr
                )
            });

            assert!(
                detail.contains(substr),
                "case `{}`: expected detail of `{}` to contain `{}`, got `{}`",
                case,
                expected.label,
                substr,
                detail
            );
        }

        if let Some(substr) = &expected.insert_text_contains {
            let insert_text = item.insert_text.as_ref().unwrap_or_else(|| {
                panic!(
                    "case `{}`: expected `{}` to have insert_text containing `{}`, but insert_text was None",
                    case, expected.label, substr
                )
            });

            assert!(
                insert_text.contains(substr),
                "case `{}`: expected insert_text of `{}` to contain `{}`, got `{}`",
                case,
                expected.label,
                substr,
                insert_text
            );
        }
    }

    if !expect.order_prefix.is_empty() {
        assert!(
            items.len() >= expect.order_prefix.len(),
            "case `{}`: expected at least {} items to verify ordering, found {}",
            case,
            expect.order_prefix.len(),
            items.len()
        );

        for (idx, expected_label) in expect.order_prefix.iter().enumerate() {
            let actual = &items[idx].label;
            assert_eq!(
                actual, expected_label,
                "case `{}`: expected item #{} to be `{}`, found `{}`",
                case, idx, expected_label, actual
            );
        }
    }

    for absent in &expect.absent {
        assert!(
            items.iter().all(|item| item.label != *absent),
            "case `{}`: expected `{}` to be absent, but it was present",
            case,
            absent
        );
    }
}
