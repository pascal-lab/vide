//! P0 measurement harness for compiler-seam costs.
//!
//! List names first: `PYTHON=/opt/homebrew/bin/python3.13 cargo test -p ide
//! --release --lib -- --list`
//!
//! Then run one named test with `--ignored --nocapture --test-threads=1`.
//! Filtering to 0 tests is not a measurement.
//!
//! slang-server is not invoked here. Numbers are Vide-only; they are not a
//! same-binary claim against slang-server.

#![cfg(test)]

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use base_db::{
    change::Change,
    project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
    source_db::SourceDb,
    source_root::{SourceRoot, SourceRootId},
};
use preproc_expand::compilation_plan;
use triomphe::Arc;
use utils::paths::AbsPathBuf;
use vfs::{ChangedFile, FileId, FileSet, VfsPath};

use crate::{
    compile::{CompileOptions, Compiler, direct_names, file_closure},
    db::root_db::RootDb,
};

fn corpus_root() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME")).join("local-devs/vide-bench/corpus")
}

fn abs(path: &Path) -> AbsPathBuf {
    AbsPathBuf::assert(
        utils::paths::Utf8PathBuf::from_path_buf(
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
        )
        .expect("utf-8 path"),
    )
}

fn walk_sources(roots: &[&Path]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack: Vec<PathBuf> = roots.iter().map(|root| (*root).to_path_buf()).collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            if matches!(ext, "sv" | "svh" | "v" | "vh") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

struct Loaded {
    db: RootDb,
    files: Vec<(FileId, PathBuf)>,
}

fn load(files: &[PathBuf], include_dirs: &[PathBuf]) -> Loaded {
    let mut file_set = FileSet::default();
    let mut change = Change::new();
    let mut loaded = Vec::new();
    for (index, path) in files.iter().enumerate() {
        let file_id = FileId::from_raw(index as u32);
        let text = fs::read_to_string(path).unwrap_or_default();
        file_set.insert(file_id, VfsPath::from(abs(path)));
        change.add_changed_file(ChangedFile::create(file_id, text.as_str()));
        loaded.push((file_id, path.clone()));
    }
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.set_project_config(Arc::new(ProjectConfig::new(
        vec![Some(CompilationProfileId(0))],
        vec![CompilationProfile {
            source_roots: vec![SourceRootId(0)],
            top_modules: Vec::new(),
            preprocess: PreprocessConfig::with_predefine_strings(
                Vec::<String>::new(),
                include_dirs.iter().map(|dir| abs(dir)).collect(),
            ),
        }],
    )));
    let mut db = RootDb::new(None);
    db.apply_change(change);
    Loaded { db, files: loaded }
}

fn find_file(loaded: &Loaded, suffix: &str) -> FileId {
    loaded
        .files
        .iter()
        .find(|(_, path)| path.to_string_lossy().ends_with(suffix))
        .map(|(file_id, _)| *file_id)
        .unwrap_or_else(|| panic!("missing {suffix}"))
}

fn lines_of(loaded: &Loaded, files: &[FileId]) -> usize {
    files
        .iter()
        .map(|file| loaded.db.file_text(*file).chars().filter(|ch| *ch == '\n').count() + 1)
        .sum()
}

fn percentile(samples: &[Duration], p: f64) -> Duration {
    let mut ordered = samples.to_vec();
    ordered.sort();
    let index = ((ordered.len() as f64 - 1.0) * p).round() as usize;
    ordered[index]
}

fn print_ms(label: &str, elapsed: Duration) {
    println!("{label}\t{:.3}ms", elapsed.as_secs_f64() * 1000.0);
}

fn virt(path: &str) -> AbsPathBuf {
    let prefix = if cfg!(windows) { r"C:\repo" } else { "/repo" };
    let sep = if cfg!(windows) { "\\" } else { "/" };
    AbsPathBuf::assert(utils::paths::Utf8PathBuf::from(format!("{prefix}{sep}{path}")))
}

fn load_virtual(entries: &[(&str, &str)]) -> Loaded {
    let mut file_set = FileSet::default();
    let mut change = Change::new();
    let mut loaded = Vec::new();
    for (index, (path, text)) in entries.iter().enumerate() {
        let file_id = FileId::from_raw(index as u32);
        file_set.insert(file_id, VfsPath::from(virt(path)));
        change.add_changed_file(ChangedFile::create(file_id, *text));
        loaded.push((file_id, PathBuf::from(path)));
    }
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.set_project_config(Arc::new(ProjectConfig::new(
        vec![Some(CompilationProfileId(0))],
        vec![CompilationProfile {
            source_roots: vec![SourceRootId(0)],
            top_modules: Vec::new(),
            preprocess: PreprocessConfig::default(),
        }],
    )));
    let mut db = RootDb::new(None);
    db.apply_change(change);
    Loaded { db, files: loaded }
}

/// Single-file cold compile, warm lookup, SourceSession tree reuse, and
/// body-only parse count. No corpus.
#[test]
#[ignore = "p0 harness: run with --release -- --ignored --nocapture --test-threads=1"]
fn p0_slang_seam_costs() {
    println!("slang-server\tnot invoked; Vide-only samples, not a same-binary claim");
    measure_single_file_cold_and_warm();
    measure_session_tree_reuse_and_body_edit();
}

fn measure_single_file_cold_and_warm() {
    println!("\n== slang cold compile and warm lookup ==");
    let src = "module top;\n  logic [7:0] x;\n  initial x = 1;\nendmodule\n";
    let loaded = load_virtual(&[("top.sv", src)]);
    let file = loaded.files[0].0;
    let path = compilation_plan::source_buffer_path(&loaded.db, file).to_string();
    let needle = src.find("x;").expect("net");
    let options = CompileOptions::for_file(&loaded.db, file);

    let mut compiler = Compiler::new();
    let started = Instant::now();
    let mut compilation = compiler.compile(&loaded.db, [file], &options);
    print_ms("slang_cold_compile", started.elapsed());
    let info = compilation.lookup_symbol(&path, needle).expect("cold lookup");
    assert!(info.type_name.contains("logic"), "{info:?}");
    println!("parse_count_cold={}", compiler.session().parse_count());
    assert!(compiler.session().parse_count() > 0, "cold compile must parse");

    const SAMPLES: usize = 21;
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let mut compilation = compiler.compile(&loaded.db, [file], &options);
        let info = compilation.lookup_symbol(&path, needle);
        samples.push(started.elapsed());
        assert!(info.is_some(), "warm lookup must bind x");
    }
    print_ms("slang_warm_lookup p50", percentile(&samples, 0.50));
    print_ms("slang_warm_lookup p95", percentile(&samples, 0.95));
    print_ms("slang_warm_lookup p99", percentile(&samples, 0.99));
    println!("parse_count_after_warm={}", compiler.session().parse_count());
    assert_eq!(compiler.session().parse_count(), 1, "warm repeats must reuse the session tree");
}

fn measure_session_tree_reuse_and_body_edit() {
    println!("\n== SourceSession tree reuse and body-only parse count ==");
    let pkg = "package pkg;\n  class leaf;\n    string m_leaf_name;\n  endclass\nendpackage\n";
    let user = "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"x\";\nendmodule\n";
    let edited = "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"y\";\nendmodule\n";
    let mut loaded = load_virtual(&[("pkg.sv", pkg), ("user.sv", user)]);
    let pkg_file = loaded.files[0].0;
    let user_file = loaded.files[1].0;
    let path = compilation_plan::source_buffer_path(&loaded.db, user_file).to_string();
    let options = CompileOptions::for_file(&loaded.db, user_file);
    let files = [pkg_file, user_file];

    let mut compiler = Compiler::new();
    let started = Instant::now();
    let mut compilation = compiler.compile(&loaded.db, files, &options);
    print_ms("session_first_compile", started.elapsed());
    let first_parses = compiler.session().parse_count();
    println!("parse_count_first={first_parses}");
    assert_eq!(first_parses, 2, "pkg.sv and user.sv");
    compilation
        .lookup_symbol(&path, user.find("m_leaf_name").expect("use"))
        .expect("first compile lookup");

    let started = Instant::now();
    let mut compilation = compiler.compile(&loaded.db, files, &options);
    print_ms("session_reuse_compile", started.elapsed());
    println!("parse_count_reuse={}", compiler.session().parse_count());
    assert_eq!(compiler.session().parse_count(), first_parses, "reuse must not reparse");
    compilation
        .lookup_symbol(&path, user.find("m_leaf_name").expect("use"))
        .expect("reused trees still bind");

    let before = compiler.session().parse_count();
    let mut change = Change::new();
    change.add_changed_file(ChangedFile::modify(user_file, edited));
    loaded.db.apply_change(change);
    let started = Instant::now();
    let mut compilation = compiler.compile(&loaded.db, files, &options);
    print_ms("body_only_user_edit_compile", started.elapsed());
    let added = compiler.session().parse_count().saturating_sub(before);
    println!("parses_on_body_edit={added}");
    assert_eq!(added, 1, "body-only user.sv edit must reparse only user.sv");
    compilation
        .lookup_symbol(&path, edited.find("m_leaf_name").expect("use"))
        .expect("body-only edit still binds");
}

/// 1280-file / 2000-wire slang parse-count entry. Salsa fold numbers live in
/// `incrementality_benches`; this prints Compiler parse counts so a later
/// radius change cannot hide behind a toy fixture.
#[test]
#[ignore = "p0 harness: run with --release -- --ignored --nocapture --test-threads=1"]
fn p0_synthetic_wire_slang_parse_count() {
    println!("slang-server\tnot invoked; Vide-only samples, not a same-binary claim");
    measure_synthetic_slang(64, 8, true);
    measure_synthetic_slang(1280, 8, true);
    measure_synthetic_slang(1, 2000, true);
}

fn measure_synthetic_slang(files: usize, wires: usize, compile_all: bool) {
    println!("\n== synthetic files={files} wires={wires} compile_all={compile_all} ==");
    let mut file_set = FileSet::default();
    let mut change = Change::new();
    let mut ids = Vec::with_capacity(files);
    for index in 0..files {
        let file_id = FileId::from_raw(index as u32);
        let mut text = format!("module m{index};\n");
        for wire in 0..wires {
            text.push_str(&format!("  wire w{wire};\n  assign w{wire} = 1'b0;\n"));
        }
        text.push_str("endmodule\n");
        file_set.insert(file_id, VfsPath::from(virt(&format!("m{index}.sv"))));
        change.add_changed_file(ChangedFile::create(file_id, text.as_str()));
        ids.push(file_id);
    }
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.set_project_config(Arc::new(ProjectConfig::new(
        vec![Some(CompilationProfileId(0))],
        vec![CompilationProfile {
            source_roots: vec![SourceRootId(0)],
            top_modules: Vec::new(),
            preprocess: PreprocessConfig::default(),
        }],
    )));
    let mut db = RootDb::new(None);
    db.apply_change(change);
    let start = ids[0];
    let options = CompileOptions::for_file(&db, start);
    let mut compiler = Compiler::new();
    let compile_files: Vec<FileId> = if compile_all { ids } else { vec![start] };
    let started = Instant::now();
    let _ = compiler.compile(&db, compile_files.iter().copied(), &options);
    print_ms("slang_compile", started.elapsed());
    println!(
        "files={files} wires={wires} compiled={} parse_count={}",
        compile_files.len(),
        compiler.session().parse_count()
    );
    assert!(compiler.session().parse_count() > 0, "synthetic compile must parse");
}

#[test]
#[ignore = "p0 corpus: run with --release -- --ignored --nocapture --test-threads=1"]
fn p0_riscv_instr_pkg_and_uvm_include() {
    println!("slang-server\tnot invoked; Vide-only samples, not a same-binary claim");
    let corpus = corpus_root();
    let riscv = corpus.join("riscv-dv");
    let uvm = corpus.join("uvm-core/src");
    assert!(riscv.is_dir(), "missing {riscv:?}");
    assert!(uvm.is_dir(), "missing {uvm:?}");

    measure_riscv_instr_pkg_closure(&riscv, &uvm);
    measure_uvm_include_into_package(&uvm);
    measure_profile_compile(&riscv);
}

fn measure_riscv_instr_pkg_closure(riscv: &Path, uvm: &Path) {
    println!("\n== keystroke compile is waitable ==");
    println!("corpus\t{}/src riscv_instr_pkg (file closure, not full UVM BCL)", riscv.display());

    let mut files = walk_sources(&[
        &riscv.join("src"),
        &riscv.join("target/rv64gc"),
        &riscv.join("user_extension"),
    ]);
    files.retain(|path| {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        name != "riscvOVPsim.ic"
    });
    let include_dirs = vec![
        riscv.join("src"),
        riscv.join("src/isa"),
        riscv.join("target/rv64gc"),
        riscv.join("user_extension"),
        uvm.to_path_buf(),
    ];
    let mut loaded = load(&files, &include_dirs);
    let pkg = find_file(&loaded, "riscv_instr_pkg.sv");
    let closure = file_closure(&loaded.db, pkg);
    let direct = direct_names(&loaded.db, pkg);
    println!(
        "workspace_files={}\tclosure_files={}\tdirect_names={}\tclosure_lines={}",
        loaded.files.len(),
        closure.files().len(),
        direct.files().len(),
        lines_of(&loaded, closure.files())
    );

    let options = CompileOptions::for_file(&loaded.db, pkg);
    let path = compilation_plan::source_buffer_path(&loaded.db, pkg).to_string();
    let text = loaded.db.file_text(pkg);
    let needle = text.find("mem_region_t").expect("mem_region_t");
    let mut compiler = Compiler::new();
    let mut compilation = compiler.compile(&loaded.db, closure.files().iter().copied(), &options);
    let warm = compilation.lookup_symbol(&path, needle);
    println!(
        "warm_lookup\tfound={}\tkind={:?}\tparse_count={}",
        warm.is_some(),
        warm.as_ref().map(|info| info.kind.clone()),
        compiler.session().parse_count()
    );

    const SAMPLES: usize = 21;
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let closure = file_closure(&loaded.db, pkg);
        let mut compilation =
            compiler.compile(&loaded.db, closure.files().iter().copied(), &options);
        let info = compilation.lookup_symbol(&path, needle);
        samples.push(started.elapsed());
        assert!(info.is_some(), "warm closure lookup must bind mem_region_t");
    }
    print_ms("compile+lookup p50", percentile(&samples, 0.50));
    print_ms("compile+lookup p95", percentile(&samples, 0.95));
    print_ms("compile+lookup p99", percentile(&samples, 0.99));
    print_ms("compile+lookup max", *samples.iter().max().expect("samples"));
    println!("parse_count_after_warm_repeats={}", compiler.session().parse_count());

    let tb = find_file(&loaded, "riscv_instr_pkg.sv");
    let original = loaded.db.file_text(tb).to_string();
    let edited = original.replacen(
        "package riscv_instr_pkg;",
        "package riscv_instr_pkg;\n  // body edit\n",
        1,
    );
    let before = compiler.session().parse_count();
    let mut change = Change::new();
    change.add_changed_file(ChangedFile::modify(tb, edited.as_str()));
    loaded.db.apply_change(change);
    let closure = file_closure(&loaded.db, tb);
    let started = Instant::now();
    let _ = compiler.compile(&loaded.db, closure.files().iter().copied(), &options);
    print_ms("body-only pkg edit compile", started.elapsed());
    let added = compiler.session().parse_count().saturating_sub(before);
    println!(
        "parses_on_body_edit={added}\t(package/BCL should be 0 extra CU parses besides the dirty file)"
    );
}

fn measure_uvm_include_into_package(uvm: &Path) {
    println!("\n== include-into-package is in the closure ==");
    println!("corpus\t{}", uvm.display());
    let mut files = walk_sources(&[uvm]);
    let tb_path = uvm.join("_vide_tb.sv");
    let tb_text = "module tb;\n  import uvm_pkg::*;\n  uvm_object obj;\nendmodule\n";
    files.push(tb_path.clone());
    let mut loaded = load(&files, &[uvm.to_path_buf()]);
    // The TB path may not exist on disk; overlay the text.
    let tb = find_file(&loaded, "_vide_tb.sv");
    let mut change = Change::new();
    change.add_changed_file(ChangedFile::modify(tb, tb_text));
    loaded.db.apply_change(change);

    let pkg = find_file(&loaded, "uvm_pkg.sv");
    let object = find_file(&loaded, "uvm_object.svh");
    let closure = file_closure(&loaded.db, tb);
    let direct = direct_names(&loaded.db, tb);
    println!(
        "tb_closure_files={}\tdirect_names={}\tcontains_uvm_pkg={}\tcontains_uvm_object={}",
        closure.files().len(),
        direct.files().len(),
        closure.files().contains(&pkg),
        closure.files().contains(&object)
    );
    assert!(closure.files().contains(&pkg), "import uvm_pkg must locate uvm_pkg.sv");
    assert!(
        closure.files().contains(&object),
        "include-into-package must put uvm_object.svh in the closure"
    );
    assert!(
        !direct.files().contains(&object),
        "slang-server direct_names must not walk uvm_pkg's include graph"
    );

    let options = CompileOptions::for_file(&loaded.db, tb);
    let path = compilation_plan::source_buffer_path(&loaded.db, tb).to_string();
    let mut compiler = Compiler::new();
    let mut compilation = compiler.compile(&loaded.db, closure.files().iter().copied(), &options);
    let info = compilation.lookup_symbol(&path, tb_text.find("uvm_object").expect("use"));
    println!(
        "tb_lookup_uvm_object\tfound={}\tkind={:?}\ttype={:?}\tparse_count={}",
        info.is_some(),
        info.as_ref().map(|info| info.kind.clone()),
        info.as_ref().map(|info| info.type_name.clone()),
        compiler.session().parse_count()
    );

    let before = compiler.session().parse_count();
    let edited =
        "module tb;\n  import uvm_pkg::*;\n  uvm_object obj;\n  initial obj = null;\nendmodule\n";
    let mut change = Change::new();
    change.add_changed_file(ChangedFile::modify(tb, edited));
    loaded.db.apply_change(change);
    let closure = file_closure(&loaded.db, tb);
    let started = Instant::now();
    let mut compilation = compiler.compile(&loaded.db, closure.files().iter().copied(), &options);
    print_ms("body-only TB edit compile", started.elapsed());
    let added = compiler.session().parse_count().saturating_sub(before);
    let info = compilation.lookup_symbol(&path, edited.find("uvm_object").expect("use"));
    println!(
        "parses_on_tb_body_edit={added}\tlookup_still_found={}\t(BCL/package CU parses should stay 0 besides tb)",
        info.is_some()
    );
}

fn measure_profile_compile(riscv: &Path) {
    println!("\n== profile compile may be slow (not a keystroke gate) ==");
    let files = walk_sources(&[&riscv.join("src"), &riscv.join("target/rv64gc")]);
    let include_dirs = vec![
        riscv.join("src"),
        riscv.join("src/isa"),
        riscv.join("target/rv64gc"),
        riscv.join("user_extension"),
    ];
    let loaded = load(&files, &include_dirs);
    let cus: Vec<FileId> = loaded
        .files
        .iter()
        .map(|(file_id, _)| *file_id)
        .filter(|file_id| loaded.db.file_kind(*file_id).is_semantic_compilation_unit())
        .collect();
    println!("profile_cu_files={}\tlines={}", cus.len(), lines_of(&loaded, &cus));
    let options = CompileOptions::for_profile(&loaded.db, Some(CompilationProfileId(0)));
    let mut compiler = Compiler::new();
    let started = Instant::now();
    let _ = compiler.compile(&loaded.db, cus.iter().copied(), &options);
    print_ms("profile compile(all CUs)", started.elapsed());
    println!("note\tthis must not run on the keystroke request path");
}
