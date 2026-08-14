//! Ignored benchmarks for the per-source-root semantic index.
//!
//! These measure the *current* architecture's costs:
//!
//! - B2 `index_build_scales_with_file_size`: cold-build cost of
//!   `SemanticIndex::for_source_root` (plus the `ModuleIndex` it pulls in) as a
//!   function of file size. A linear-resolver design should cost O(bytes);
//!   super-linear growth points at per-token scans.
//! - B3 `index_rebuild_after_single_file_change`: after touching one small file
//!   in a root, the cost of re-serving the root index. If this is close to the
//!   cold-build cost, the whole root is re-resolved on every change.
//!
//! Run with:
//!
//! ```text
//! cargo test -p ide --release -- --ignored --nocapture index_benchmarks
//! ```

use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use base_db::{
    change::Change,
    project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
    source_db::SourceRootDb,
    source_root::{SourceRoot, SourceRootId},
};
use triomphe::Arc;
use utils::{
    line_index::{TextRange, TextSize},
    paths::abs_path_buf_from_path_buf,
};
use vfs::{AbsPathBuf, ChangedFile, FileId, FileSet, PathMatcher, VfsPath};

use crate::{
    FilePosition, ScopeVisibility,
    analysis_host::AnalysisHost,
    db::workspace_symbol_index_db::{
        source_root_module_index_for_root, source_root_semantic_index_for_root,
    },
    document_highlight::DocumentHighlightConfig,
    goto_definition,
    references::ReferencesConfig,
    semantic_index::{incoming_module_edges, outgoing_module_edges},
    test_utils::normalize_fixture_text,
};

/// One repeated module body; roughly 130 bytes with ~15 name-like tokens.
fn module_text(name: u32) -> String {
    format!(
        "module m{name}(input logic clk);\n  logic a{name}, b{name};\n  assign a{name} = b{name} ^ clk;\n  always_ff @(posedge clk) b{name} <= a{name};\nendmodule\n\n"
    )
}

/// A file dominated by macro expansions: one object-like macro emitting a
/// full module body, invoked once per generated module. Every expanded token
/// resolves inside a macro region, so this exercises the shared emitted-token
/// index path of `collect_file`.
fn macro_dense_text(modules: u32) -> String {
    let mut text = String::from(
        "`define GEN(n) module m{n}(input logic clk);\n  logic a{n}, b{n};\n  assign a{n} = b{n} ^ clk;\n  always_ff @(posedge clk) b{n} <= a{n};\nendmodule\n",
    );
    for n in 0..modules {
        text.push_str(&format!("`GEN({n})\n"));
    }
    text
}

fn file_text(modules: u32) -> String {
    (0..modules).map(module_text).collect()
}

fn bytes_of(modules: u32) -> usize {
    file_text(modules).len()
}

fn host_with_single_file(text: &str) -> (AnalysisHost, FileId) {
    let text = normalize_fixture_text(text);
    let file_id = FileId::from_raw(0);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new_virtual_path("/bench.sv".to_owned()));
    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.add_changed_file(ChangedFile::create(file_id, text.as_str()));
    let mut host = AnalysisHost::default();
    host.apply_change(change);
    (host, file_id)
}

fn timed<F: FnOnce() -> T, T>(f: F) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

#[test]
#[ignore]
fn index_benchmarks_macro_dense_build() {
    let counts = [128u32, 256, 512, 1024];
    println!("\n== B4: cold SemanticIndex build, macro-dense file (release) ==");
    println!("{:<10} {:<10} {:<14}", "calls", "bytes", "semantic_idx");
    for count in counts {
        let text = macro_dense_text(count);
        let (host, file_id) = host_with_single_file(&text);
        let db = host.raw_db();
        let root_id = db.source_root_id(file_id);
        let (_, semantic_cost) =
            timed(|| std::hint::black_box(source_root_semantic_index_for_root(db, root_id)));
        println!("{:<10} {:<10} {:<14?}", count, text.len(), semantic_cost);
    }
}

#[test]
#[ignore]
fn index_benchmarks_build_scales_with_file_size() {
    let modules = [32usize, 64, 128, 256, 512, 1024];
    println!("\n== B2: cold SemanticIndex + ModuleIndex build vs file size (release) ==");
    println!("{:<10} {:<10} {:<14} {:<14}", "modules", "bytes", "module_idx", "semantic_idx");
    for count in modules {
        let text = file_text(count as u32);
        let (host, file_id) = host_with_single_file(&text);
        let db = host.raw_db();
        let root_id = db.source_root_id(file_id);

        let (_, module_cost) =
            timed(|| std::hint::black_box(source_root_module_index_for_root(db, root_id)));
        let (_, semantic_cost) =
            timed(|| std::hint::black_box(source_root_semantic_index_for_root(db, root_id)));

        println!(
            "{:<10} {:<10} {:<14?} {:<14?}",
            count,
            bytes_of(count as u32),
            module_cost,
            semantic_cost
        );
    }
}

/// Real-file soak test: loads `$VIDE_BENCH_FILE` as a single-file root and
/// times the cold parse, module index, semantic index, one representative
/// request of each navigation feature, and the incremental rebuild after a
/// one-byte touch at the end of the file.
///
/// Set `$VIDE_BENCH_PROBE` to a module identifier when the file does not use
/// the fixture's default `array_0_ext` probe.
///
/// Run with:
///
/// ```text
/// VIDE_BENCH_FILE=~/Downloads/XS.v VIDE_BENCH_PROBE=top \
///   cargo test -p ide --release -- --ignored --nocapture index_benchmarks_real_file
/// ```
#[test]
#[ignore]
fn index_benchmarks_real_file() {
    let Some(path) = std::env::var_os("VIDE_BENCH_FILE") else {
        println!("VIDE_BENCH_FILE not set; skipping real-file benchmark");
        return;
    };
    let path = std::path::PathBuf::from(path);
    let text = fs::read_to_string(&path).expect("read benchmark file");
    let line_count = text.lines().count();
    eprintln!(
        "\n== B5: real-file soak test ({path:?}, {line_count} lines, {} bytes) ==",
        text.len()
    );

    let file_id = FileId::from_raw(0);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new_virtual_path("/XS.v".to_owned()));
    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.add_changed_file(ChangedFile::create(file_id, text.as_str()));
    let mut host = AnalysisHost::default();
    host.apply_change(change);

    let db = host.raw_db();
    let root_id = db.source_root_id(file_id);

    let (_, parse_cost) = timed(|| std::hint::black_box(db.parse(file_id.into())));
    eprintln!("cold parse:                                    {parse_cost:?}");

    let (_, module_cost) =
        timed(|| std::hint::black_box(source_root_module_index_for_root(db, root_id)));
    eprintln!("module index:                                  {module_cost:?}");

    let (_, semantic_cost) =
        timed(|| std::hint::black_box(source_root_semantic_index_for_root(db, root_id)));
    eprintln!("semantic index (cold, first build):           {semantic_cost:?}");

    let probe = std::env::var("VIDE_BENCH_PROBE").unwrap_or_else(|_| "array_0_ext".to_owned());
    let probe_offset = TextSize::from(
        u32::try_from(
            text.find(&probe).unwrap_or_else(|| panic!("probe module {probe:?} should exist")),
        )
        .unwrap(),
    );
    let position = FilePosition { file_id, offset: probe_offset };

    let (nav, goto_cost) = timed(|| goto_definition::goto_definition(db, position));
    eprintln!(
        "goto definition on first module ({probe}):     {goto_cost:?} ({} targets)",
        nav.map_or(0, |info| info.info.len())
    );

    let (highlights, highlight_cost) = timed(|| {
        crate::document_highlight::document_highlight(
            db,
            position,
            DocumentHighlightConfig { scope_visibility: ScopeVisibility::Public },
        )
    });
    eprintln!(
        "document highlight:                            {highlight_cost:?} ({} highlights)",
        highlights.map_or(0, |h| h.len())
    );

    let (refs, refs_cost) = timed(|| {
        crate::references::references(
            db,
            position,
            ReferencesConfig::new(ScopeVisibility::Public, None),
        )
    });
    let ref_count =
        refs.map_or(0, |rs| rs.iter().map(|r| r.refs.values().map(Vec::len).sum::<usize>()).sum());
    eprintln!("find references (workspace):                  {refs_cost:?} ({ref_count} refs)");

    let probe_range = TextRange::new(probe_offset, probe_offset + TextSize::of(&probe));
    let (incoming, incoming_cost) = timed(|| incoming_module_edges(db, file_id, probe_range));
    eprintln!(
        "call hierarchy incoming:                      {incoming_cost:?} ({} edges)",
        incoming.len()
    );
    let (outgoing, outgoing_cost) = timed(|| outgoing_module_edges(db, file_id, probe_range));
    eprintln!(
        "call hierarchy outgoing:                      {outgoing_cost:?} ({} edges)",
        outgoing.len()
    );

    // One-byte touch at the end of the file, then rebuild.
    let mut touch = Change::new();
    let touched = format!("{text} ");
    touch.add_changed_file(ChangedFile::create(file_id, touched.as_str()));
    host.apply_change(touch);
    let db = host.raw_db();
    let (_, rebuild_cost) =
        timed(|| std::hint::black_box(source_root_semantic_index_for_root(db, root_id)));
    eprintln!("semantic index (rebuild after one-byte touch): {rebuild_cost:?}");
}

/// Micro-benchmark separating the per-token nameres costs: the salsa
/// `scope_for` hit, the `NameScope::lookup` hash, and the `ScopeParent` walk.
/// Debug instrumentation for the index-build fast path.
#[test]
#[ignore]
fn index_benchmarks_nameres_primitives() {
    println!("retired: superseded by the scope-chain fast path");
}

#[test]
#[ignore]
fn index_benchmarks_rebuild_after_single_file_change() {
    println!("\n== B3: root index rebuild after touching one small file (release) ==");

    let big_text = file_text(512); // ~64 KB
    let small_text = "module small;\n  logic s;\nendmodule\n";

    let big_file = FileId::from_raw(0);
    let small_file = FileId::from_raw(1);
    let mut file_set = FileSet::default();
    file_set.insert(big_file, VfsPath::new_virtual_path("/big.sv".to_owned()));
    file_set.insert(small_file, VfsPath::new_virtual_path("/small.sv".to_owned()));

    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.add_changed_file(ChangedFile::create(big_file, big_text.as_str()));
    change.add_changed_file(ChangedFile::create(small_file, small_text));
    let mut host = AnalysisHost::default();
    host.apply_change(change);

    let db = host.raw_db();
    let root_id = db.source_root_id(big_file);

    let (_, cold) =
        timed(|| std::hint::black_box(source_root_semantic_index_for_root(db, root_id)));
    println!("cold build of root (64KB big file + small file): {cold:?}");

    // Touch only the small file: append a comment.
    let mut touch = Change::new();
    touch.add_changed_file(ChangedFile::create(
        small_file,
        "module small;\n  logic s; // touched\nendmodule\n",
    ));
    host.apply_change(touch);

    let db = host.raw_db();
    let (_, rebuild) =
        timed(|| std::hint::black_box(source_root_semantic_index_for_root(db, root_id)));
    println!("rebuild after touching only the small file:     {rebuild:?}");

    // Lower bound: building an index for a root containing only the small
    // file. If `rebuild` is close to `cold` instead of close to this, the
    // whole root is re-resolved on every change.
    let mut single_set = FileSet::default();
    single_set.insert(small_file, VfsPath::new_virtual_path("/small.sv".to_owned()));
    let mut single_change = Change::new();
    single_change.set_roots(vec![SourceRoot::new_local(single_set)]);
    single_change.add_changed_file(ChangedFile::create(
        small_file,
        "module small;\n  logic s; // touched\nendmodule\n",
    ));
    let mut single_host = AnalysisHost::default();
    single_host.apply_change(single_change);
    let single_db = single_host.raw_db();
    let single_root = single_db.source_root_id(small_file);
    let (_, lower_bound) =
        timed(|| std::hint::black_box(source_root_semantic_index_for_root(single_db, single_root)));
    println!("lower bound (indexing only the small file alone): {lower_bound:?}");
}

/// Load a real SystemVerilog project directory into a fresh [`AnalysisHost`].
///
/// Every source file under `root` (`.v/.sv/.vh/.svh/.svi/.map`) is discovered,
/// read, and registered in a single local [`SourceRoot`]. `root` doubles as the
/// only include directory so relative `` `include `` directives resolve.
///
/// Returns the host, the loaded [`FileId`]s, total bytes, and total newlines.
///
/// `.map` library-map files are excluded from the indexed source set: they
/// parse to a `LibraryMap` syntax root, which the item-tree path behind the
/// semantic index does not accept (it requires a compilation unit).
///
/// NOTE: this simplified walk does not exclude `.git`/`target`/`build`. That is
/// fine for clean fixture dirs (e.g. slang's `tests/unittests/data`); for large
/// real repos the server's `get_workspace_folder` exclude policy should be
/// reused instead.
fn host_with_project(root: &AbsPathBuf) -> (AnalysisHost, Vec<FileId>, usize, usize) {
    let files = PathMatcher::all_under_roots(vec![root.clone()])
        .collect_matching_files(vfs::loader::SOURCE_FILE_EXTENSIONS)
        .into_iter()
        .filter(|path| {
            !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("map"))
        })
        .collect::<Vec<_>>();

    let mut file_set = FileSet::default();
    let mut changed_files = Vec::with_capacity(files.len());
    let mut file_ids = Vec::with_capacity(files.len());
    let mut total_bytes = 0usize;
    let mut total_lines = 0usize;

    for (idx, path) in files.into_iter().enumerate() {
        let Ok(text) = fs::read_to_string(path.as_path()) else {
            continue;
        };
        total_bytes += text.len();
        total_lines += text.bytes().filter(|byte| *byte == b'\n').count();
        let file_id = FileId::from_raw(u32::try_from(idx).expect("bench file index fits u32"));
        file_set.insert(file_id, VfsPath::from(path));
        changed_files.push(ChangedFile::create(file_id, text.as_str()));
        file_ids.push(file_id);
    }

    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.set_project_config(Arc::new(ProjectConfig::new(
        vec![Some(CompilationProfileId(0))],
        vec![CompilationProfile {
            source_roots: vec![SourceRootId(0)],
            top_modules: Vec::new(),
            preprocess: PreprocessConfig {
                include_dirs: vec![root.clone()],
                ..PreprocessConfig::default()
            },
        }],
    )));
    for changed_file in changed_files {
        change.add_changed_file(changed_file);
    }

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    (host, file_ids, total_bytes, total_lines)
}

/// Real multi-file project benchmark: loads `$VIDE_BENCH_PROJECT` as one source
/// root and times cold load, cold parse, module index, semantic index, and the
/// semantic-index rebuild after touching one file.
///
/// Run with:
///
/// ```text
/// VIDE_BENCH_PROJECT=third_party/slang/tests/unittests/data \
///   cargo test -p ide --release -- --ignored --nocapture index_benchmarks_real_project
/// ```
#[test]
#[ignore]
fn index_benchmarks_real_project() {
    let Some(raw) = std::env::var_os("VIDE_BENCH_PROJECT") else {
        println!("VIDE_BENCH_PROJECT not set; skipping real-project benchmark");
        return;
    };
    let Some(root) = abs_path_buf_from_path_buf(PathBuf::from(raw)) else {
        println!("VIDE_BENCH_PROJECT must be an absolute UTF-8 path");
        return;
    };

    eprintln!("\n== B6: real multi-file project ({root}) ==");

    let ((mut host, file_ids, total_bytes, total_lines), load_cost) =
        timed(|| host_with_project(&root));
    if file_ids.is_empty() {
        println!("no SystemVerilog source files found under {root}");
        return;
    }
    let file_count = file_ids.len();
    let db = host.raw_db();
    let root_id = db.source_root_id(file_ids[0]);

    eprintln!("files: {file_count}, bytes: {total_bytes}, lines: {total_lines}");
    eprintln!("cold load (discover + read + register):           {load_cost:?}");

    let (_, parse_cost) = timed(|| {
        for &file_id in &file_ids {
            std::hint::black_box(db.parse(file_id.into()));
        }
    });
    eprintln!("cold parse (all {file_count} files):                  {parse_cost:?}");

    let (_, module_cost) =
        timed(|| std::hint::black_box(source_root_module_index_for_root(db, root_id)));
    eprintln!("module index:                                      {module_cost:?}");

    let (_, semantic_cost) =
        timed(|| std::hint::black_box(source_root_semantic_index_for_root(db, root_id)));
    eprintln!("semantic index (cold, first build):               {semantic_cost:?}");

    // Incremental: touch one file, then rebuild the semantic index.
    let touch_file = file_ids[0];
    let touched_text = format!("{} // bench-touch\n", db.file_text(touch_file));
    let mut touch = Change::new();
    touch.add_changed_file(ChangedFile::create(touch_file, touched_text.as_str()));
    host.apply_change(touch);
    let db = host.raw_db();
    let (_, rebuild_cost) =
        timed(|| std::hint::black_box(source_root_semantic_index_for_root(db, root_id)));
    eprintln!("semantic index (rebuild after touching one file): {rebuild_cost:?}");
}
