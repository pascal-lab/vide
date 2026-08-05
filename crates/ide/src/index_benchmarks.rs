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

use std::time::{Duration, Instant};

use base_db::{change::Change, source_db::SourceRootDb, source_root::SourceRoot};
use vfs::{ChangedFile, FileId, FileSet, VfsPath};

use crate::{
    analysis_host::AnalysisHost,
    db::workspace_symbol_index_db::{
        source_root_module_index_for_root, source_root_semantic_index_for_root,
    },
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
