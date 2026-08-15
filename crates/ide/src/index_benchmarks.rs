//! Ignored benchmarks for the per-source-root semantic index.
//!
//! These measure the *current* architecture's costs:
//!
//! - B2 `index_build_scales_with_file_size`: cold-build cost of
//!   `ReferenceIndex::for_source_root` (plus the `ModuleIndex` it pulls in) as a
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
    completion,
    db::root_db::RootDb,
    db::workspace_symbol_index_db::{
        source_root_module_index_for_root, source_root_reference_index_for_root,
    },
    document_highlight::DocumentHighlightConfig,
    goto_definition,
    references::ReferencesConfig,
    rename::{self, RenameConfig},
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
            timed(|| std::hint::black_box(source_root_reference_index_for_root(db, root_id)));
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
            timed(|| std::hint::black_box(source_root_reference_index_for_root(db, root_id)));

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
        timed(|| std::hint::black_box(source_root_reference_index_for_root(db, root_id)));
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
        timed(|| std::hint::black_box(source_root_reference_index_for_root(db, root_id)));
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
        timed(|| std::hint::black_box(source_root_reference_index_for_root(db, root_id)));
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
        timed(|| std::hint::black_box(source_root_reference_index_for_root(db, root_id)));
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
        timed(|| std::hint::black_box(source_root_reference_index_for_root(single_db, single_root)));
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
/// NOTE: this simplified walk does not exclude `.git`/`target`/`build`. That is
/// fine for clean fixture dirs (e.g. slang's `tests/unittests/data`); for large
/// real repos the server's `get_workspace_folder` exclude policy should be
/// reused instead.
fn host_with_project(root: &AbsPathBuf) -> (AnalysisHost, Vec<FileId>, usize, usize) {
    let files = PathMatcher::all_under_roots(vec![root.clone()])
        .collect_matching_files(vfs::loader::SOURCE_FILE_EXTENSIONS);

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

fn project_probe_position(
    db: &RootDb,
    file_ids: &[FileId],
    probe: &str,
    prefer_use: bool,
) -> Option<FilePosition> {
    let is_ident = |ch: char| ch == '_' || ch.is_ascii_alphanumeric();
    if prefer_use {
        for &file_id in file_ids {
            let text = db.file_text(file_id);
            for (start, _) in text.match_indices(probe) {
                let before = text[..start].chars().next_back();
                let after = text[start + probe.len()..].chars().next();
                if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
                    continue;
                }
                let line_prefix = text[..start].rsplit_once('\n').map_or(&text[..start], |(_, line)| line);
                let trimmed = line_prefix.trim_start();
                if trimmed.starts_with("//") || trimmed.ends_with("module ") {
                    continue;
                }
                let line_suffix = text[start + probe.len()..]
                    .split_once('\n')
                    .map_or(&text[start + probe.len()..], |(line, _)| line);
                if !line_suffix.trim_start().starts_with('#') {
                    continue;
                }
                return Some(FilePosition {
                    file_id,
                    offset: TextSize::from(u32::try_from(start).ok()?),
                });
            }
        }
    }

    let declaration = format!("module {probe}");
    for &file_id in file_ids {
        let text = db.file_text(file_id);
        for (start, _) in text.match_indices(&declaration) {
            let after = text[start + declaration.len()..].chars().next();
            if after.is_some_and(is_ident) {
                continue;
            }
            let offset = start + "module ".len();
            return Some(FilePosition {
                file_id,
                offset: TextSize::from(u32::try_from(offset).ok()?),
            });
        }
    }

    for &file_id in file_ids {
        let text = db.file_text(file_id);
        for (start, _) in text.match_indices(probe) {
            let before = text[..start].chars().next_back();
            let after = text[start + probe.len()..].chars().next();
            if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
                return Some(FilePosition {
                    file_id,
                    offset: TextSize::from(u32::try_from(start).ok()?),
                });
            }
        }
    }
    None
}

fn benchmark_project_request(
    root: &AbsPathBuf,
    probe: &str,
    label: &str,
    prefer_use: bool,
    offset_delta: TextSize,
    mut request: impl FnMut(&RootDb, FilePosition) -> usize,
) {
    const WARM_RUNS: usize = 20;

    let (mut host, file_ids, _, _) = host_with_project(root);
    let db = host.raw_db();
    let Some(mut position) = project_probe_position(db, &file_ids, probe, prefer_use) else {
        eprintln!("{label:<28} probe {probe:?} not found");
        return;
    };
    position.offset += offset_delta;

    let (cold_count, cold) = timed(|| std::hint::black_box(request(db, position)));
    let mut warm = Vec::with_capacity(WARM_RUNS);
    for _ in 0..WARM_RUNS {
        let (count, cost) = timed(|| std::hint::black_box(request(db, position)));
        assert_eq!(count, cold_count, "{label} changed its result count after warming");
        warm.push(cost);
    }
    warm.sort_unstable();
    let warm_median = warm[WARM_RUNS / 2];
    let warm_max = warm[WARM_RUNS - 1];

    let touch_file = file_ids[0];
    let touched_text = format!("{} // request-bench-touch\n", db.file_text(touch_file));
    let mut touch = Change::new();
    touch.add_changed_file(ChangedFile::create(touch_file, touched_text.as_str()));
    host.apply_change(touch);
    let db = host.raw_db();
    let (after_edit_count, after_edit) = timed(|| std::hint::black_box(request(db, position)));
    assert_eq!(
        after_edit_count, cold_count,
        "{label} changed its result count after an unrelated body-only edit"
    );

    eprintln!(
        "{label:<28} cold={cold:?} warm(p50/max)={warm_median:?}/{warm_max:?} after-edit={after_edit:?} results={cold_count}/{after_edit_count}"
    );
}

/// End-to-end latency of representative IDE requests on a real multi-file
/// project. Each request gets a fresh host, so `cold` includes its own query
/// and index population rather than inheriting caches from an earlier feature.
///
/// `VIDE_BENCH_PROBE` should name a module with cross-file uses; common_cells
/// defaults to `cc_fifo`.
///
/// ```text
/// VIDE_BENCH_PROJECT=/tmp/vide-bench/common_cells \
///   cargo test -p ide --release --lib -- --ignored --nocapture \
///   index_benchmarks_real_project_requests
/// ```
#[test]
#[ignore]
fn index_benchmarks_real_project_requests() {
    let Some(raw) = std::env::var_os("VIDE_BENCH_PROJECT") else {
        println!("VIDE_BENCH_PROJECT not set; skipping real-project request benchmark");
        return;
    };
    let Some(root) = abs_path_buf_from_path_buf(PathBuf::from(raw)) else {
        println!("VIDE_BENCH_PROJECT must be an absolute UTF-8 path");
        return;
    };
    let probe = std::env::var("VIDE_BENCH_PROBE").unwrap_or_else(|_| "cc_fifo".to_owned());
    eprintln!("\n== B7: real-project IDE requests ({root}, probe={probe}) ==");

    benchmark_project_request(&root, &probe, "goto definition", true, TextSize::from(0), |db, position| {
        goto_definition::goto_definition(db, position).map_or(0, |info| info.info.len())
    });
    benchmark_project_request(&root, &probe, "document highlight", true, TextSize::from(0), |db, position| {
        crate::document_highlight::document_highlight(
            db,
            position,
            DocumentHighlightConfig { scope_visibility: ScopeVisibility::Public },
        )
        .map_or(0, |items| items.len())
    });
    benchmark_project_request(&root, &probe, "find references", true, TextSize::from(0), |db, position| {
        crate::references::references(
            db,
            position,
            ReferencesConfig::new(ScopeVisibility::Public, None),
        )
        .map_or(0, |groups| {
            groups.iter().map(|group| group.refs.values().map(Vec::len).sum::<usize>()).sum()
        })
    });
    benchmark_project_request(&root, &probe, "rename edit generation", true, TextSize::from(0), |db, position| {
        rename::rename(
            db,
            position,
            RenameConfig::workspace(ScopeVisibility::Public),
            "vide_bench_renamed",
        )
        .map_or(0, |change| change.text_edits.len())
    });
    let completion_prefix = TextSize::from(u32::try_from(probe.len().min(3)).unwrap());
    benchmark_project_request(&root, &probe, "completion", true, completion_prefix, |db, position| {
        completion::completions(db, position, None).len()
    });
    benchmark_project_request(&root, &probe, "call hierarchy incoming", false, TextSize::from(0), |db, position| {
        let range = TextRange::new(position.offset, position.offset + TextSize::of(probe.as_str()));
        incoming_module_edges(db, position.file_id, range).len()
    });
    benchmark_project_request(&root, &probe, "call hierarchy outgoing", false, TextSize::from(0), |db, position| {
        let range = TextRange::new(position.offset, position.offset + TextSize::of(probe.as_str()));
        outgoing_module_edges(db, position.file_id, range).len()
    });
}

/// Separates `$unit` scope memo validation from the owner-table dependencies
/// it validates after an unrelated edit. The two hosts start from identical
/// cold state: the first measures `unit_scope` directly, while the second
/// validates every owner table before asking for `unit_scope`.
#[test]
#[ignore]
fn index_benchmarks_real_project_unit_scope_validation() {
    let Some(raw) = std::env::var_os("VIDE_BENCH_PROJECT") else {
        println!("VIDE_BENCH_PROJECT not set; skipping unit-scope validation benchmark");
        return;
    };
    let Some(root) = abs_path_buf_from_path_buf(PathBuf::from(raw)) else {
        println!("VIDE_BENCH_PROJECT must be an absolute UTF-8 path");
        return;
    };

    let prepare = || {
        let (mut host, file_ids, _, _) = host_with_project(&root);
        let db = host.raw_db();
        std::hint::black_box(db.unit_scope());
        let touch_file = file_ids[0];
        let touched_text = format!("{} // unit-scope-bench-touch\n", db.file_text(touch_file));
        let mut touch = Change::new();
        touch.add_changed_file(ChangedFile::create(touch_file, touched_text.as_str()));
        host.apply_change(touch);
        (host, file_ids)
    };

    let (direct_host, _) = prepare();
    let (_, direct) = timed(|| std::hint::black_box(direct_host.raw_db().unit_scope()));

    let (owner_host, file_ids) = prepare();
    let db = owner_host.raw_db();
    let (_, owner_tables) = timed(|| {
        for &file_id in &file_ids {
            std::hint::black_box(db.owner_table(preproc_expand::file::HirFileId::File(file_id)));
        }
    });
    let (_, after_owner_tables) = timed(|| std::hint::black_box(db.unit_scope()));

    eprintln!("\n== B8: real-project unit-scope validation ({root}) ==");
    eprintln!("unit_scope directly after edit:        {direct:?}");
    eprintln!("validate all owner tables after edit: {owner_tables:?}");
    eprintln!("unit_scope after owner tables:        {after_owner_tables:?}");
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
        timed(|| std::hint::black_box(source_root_reference_index_for_root(db, root_id)));
    eprintln!("semantic index (cold, first build):               {semantic_cost:?}");

    // Incremental: touch one file, then rebuild the semantic index.
    let touch_file = file_ids[0];
    let touched_text = format!("{} // bench-touch\n", db.file_text(touch_file));
    let mut touch = Change::new();
    touch.add_changed_file(ChangedFile::create(touch_file, touched_text.as_str()));
    host.apply_change(touch);
    let db = host.raw_db();
    let (_, rebuild_cost) =
        timed(|| std::hint::black_box(source_root_reference_index_for_root(db, root_id)));
    eprintln!("semantic index (rebuild after touching one file): {rebuild_cost:?}");
}

/// Debug instrumentation for the module-index build path: decomposes the
/// per-file costs into parse, macro-file discovery, AST id map, owner table,
/// and the item-tree residual.
///
/// Each query is timed after its inputs are warm, so the numbers are the
/// *incremental* cost of that query, not cold wall-clock.
///
/// Run with:
///
/// ```text
/// VIDE_BENCH_PROJECT=third_party/slang/tests/unittests/data \
///   cargo test -p ide --release --lib -- --ignored --nocapture index_benchmarks_module_index_profile
/// ```
#[test]
#[ignore]
fn index_benchmarks_module_index_profile() {
    use hir_def::db::HirDefDb;
    use preproc_expand::{db::PreprocDb, file::HirFileId, macro_file::macro_files_for_file};

    let Some(raw) = std::env::var_os("VIDE_BENCH_PROJECT") else {
        println!("VIDE_BENCH_PROJECT not set; skipping module-index profile");
        return;
    };
    let Some(root) = abs_path_buf_from_path_buf(PathBuf::from(raw)) else {
        println!("VIDE_BENCH_PROJECT must be an absolute UTF-8 path");
        return;
    };
    let (host, file_ids, _, _) = host_with_project(&root);
    if file_ids.is_empty() {
        println!("no SystemVerilog source files found under {root}");
        return;
    }
    let db = host.raw_db();

    let mut parse_cost = Duration::ZERO;
    let mut macro_cost = Duration::ZERO;
    let mut ast_id_cost = Duration::ZERO;
    let mut owner_cost = Duration::ZERO;
    let mut item_tree_cost = Duration::ZERO;

    // Cold parse first; every query below reuses the parse cache.
    for &file_id in &file_ids {
        let (_, cost) = timed(|| std::hint::black_box(db.parse(file_id.into())));
        parse_cost += cost;
    }
    for &file_id in &file_ids {
        let (_, cost) = timed(|| macro_files_for_file(db, file_id));
        macro_cost += cost;
    }
    for &file_id in &file_ids {
        let hir_file_id = HirFileId::File(file_id);
        let (_, cost) = timed(|| std::hint::black_box(db.ast_id_map(hir_file_id)));
        ast_id_cost += cost;
    }
    for &file_id in &file_ids {
        let hir_file_id = HirFileId::File(file_id);
        let (_, cost) = timed(|| std::hint::black_box(db.owner_table(hir_file_id)));
        owner_cost += cost;
    }
    for &file_id in &file_ids {
        let hir_file_id = HirFileId::File(file_id);
        let (_, cost) = timed(|| std::hint::black_box(db.item_tree(hir_file_id)));
        item_tree_cost += cost;
    }

    eprintln!("\n== module-index profile ({root}) ==");
    eprintln!("files: {}", file_ids.len());
    eprintln!("parse (cold):        {parse_cost:?}");
    eprintln!("macro_files_for_file:{macro_cost:?}");
    eprintln!("ast_id_map:          {ast_id_cost:?}");
    eprintln!("owner_table:         {owner_cost:?}");
    eprintln!("item_tree (residual):{item_tree_cost:?}");

    // Isolate the full-profile slang compilation (`parsed_profile`): cold
    // first call vs a warm second call in a fresh host.
    {
        let (host, ids, _, _) = host_with_project(&root);
        let db = host.raw_db();
        let (_, cold) = timed(|| std::hint::black_box(db.parse_tree(ids[0])));
        eprintln!("parsed_compilation_unit (cold): {cold:?}");
        let warm = ids.get(1).copied().map(|file_id| {
            let (_, cost) = timed(|| std::hint::black_box(db.parse_tree(file_id)));
            cost
        });
        if let Some(warm) = warm {
            eprintln!("parsed_compilation_unit (warm): {warm:?}");
        }
    }

    // The remaining macro_files_for_file sub-queries, each cold in a fresh
    // host so no earlier measurement warms them.
    {
        let (host, ids, _, _) = host_with_project(&root);
        let db = host.raw_db();
        let mut cost = Duration::ZERO;
        for &file_id in &ids {
            let (_, c) =
                timed(|| std::hint::black_box(db.source_preproc_contexts_for_file(file_id)));
            cost += c;
        }
        eprintln!("source_preproc_contexts_for_file: {cost:?}");
    }
    {
        let (host, ids, _, _) = host_with_project(&root);
        let db = host.raw_db();
        let mut cost = Duration::ZERO;
        for &file_id in &ids {
            let (_, c) = timed(|| std::hint::black_box(db.source_preproc_model(file_id)));
            cost += c;
        }
        eprintln!("source_preproc_model: {cost:?}");
    }
    {
        let (host, ids, _, _) = host_with_project(&root);
        let db = host.raw_db();
        let mut cost = Duration::ZERO;
        for &file_id in &ids {
            let (_, c) = timed(|| std::hint::black_box(db.trace_index(file_id)));
            cost += c;
        }
        eprintln!("trace_index: {cost:?}");
    }

    // The semantic-index per-file queries (cold, in a fresh host).
    {
        let (host, ids, _, _) = host_with_project(&root);
        let db = host.raw_db();
        let mut sem_cost = Duration::ZERO;
        let mut edges_cost = Duration::ZERO;
        let mut per_file = Vec::new();
        for &file_id in &ids {
            let (_, s) = timed(|| std::hint::black_box(db.file_semantic_index(file_id)));
            let (_, e) = timed(|| std::hint::black_box(db.file_module_edges(file_id)));
            sem_cost += s;
            edges_cost += e;
            per_file.push((file_id, s, e));
        }
        eprintln!("file_semantic_index (sum): {sem_cost:?}");
        eprintln!("file_module_edges (sum):   {edges_cost:?}");
        per_file.sort_by_key(|&(_, s, _)| std::cmp::Reverse(s));
        for (file_id, s, e) in per_file.into_iter().take(10) {
            eprintln!("  sem per-file {s:?} edges={e:?} {:?}", db.file_path(file_id));
        }
    }
}
