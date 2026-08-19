//! Synthetic incrementality benches. Run with:
//! `cargo test -p ide --release --lib incrementality_benches -- --ignored
//! --nocapture`

use std::time::Instant;

use base_db::{change::Change, source_root::SourceRoot};
use vfs::{ChangedFile, FileId, FileSet, VfsPath};

use crate::{FilePosition, analysis_host::AnalysisHost};

/// Heavier than `module mN; endmodule` so a file_facts LRU miss is not free.
/// Eight assignments is still synthetic, but it is enough to tell a memo hit
/// from a re-extract. The previous one-line corpus hid the parse LRU behind
/// a fold that never re-queried.
fn module_text(index: usize) -> String {
    let mut text = format!("module m{index};\n");
    for wire in 0..8 {
        text.push_str(&format!("  wire w{wire};\n  assign w{wire} = 1'b0;\n"));
    }
    text.push_str("endmodule\n");
    text
}

/// Drop joins an in-flight prewarm. An empty change cancels that worker
/// without starting another, so the bench process cannot hang on join.
fn finish(host: &mut AnalysisHost) {
    host.apply_change(Change::new());
}

fn workspace_with_modules(n: usize) -> AnalysisHost {
    let mut file_set = FileSet::default();
    let mut change = Change::new();
    for index in 0..n {
        let file_id = FileId::from_raw(index as u32);
        file_set.insert(file_id, VfsPath::new_virtual_path(format!("/m{index}.sv")));
        change.add_changed_file(ChangedFile::create(file_id, module_text(index)));
    }
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    let mut host = AnalysisHost::default();
    host.apply_change(change);
    host
}

fn print_ms(label: &str, files: usize, elapsed: std::time::Duration) {
    println!("{label}\tfiles={files}\t{:.3}ms", elapsed.as_secs_f64() * 1000.0);
}

#[test]
#[ignore = "run with --release -- --ignored --nocapture"]
fn design_graph_fold_by_workspace_size() {
    for files in [64, 256, 1024, 1280] {
        let mut host = workspace_with_modules(files);
        let started = Instant::now();
        let graph = host.ctx().unit_catalog();
        print_ms("design_graph.fold", files, started.elapsed());
        assert_eq!(graph.node_count(), files);
        finish(&mut host);
    }
}

/// Salsa LRU evicts at the start of a new revision, not during a fold.
/// Capacity 2, three files, touch 0 then 1 then 2, edit file 2: file 0 is
/// the victim, file 1 stays and is still valid.
#[test]
#[ignore = "run with --release -- --ignored --nocapture"]
fn file_facts_lru_miss_is_not_free() {
    fn large_module(index: usize) -> String {
        let mut text = format!("module m{index};\n");
        for wire in 0..2000 {
            text.push_str(&format!("  wire w{wire};\n  assign w{wire} = 1'b0;\n"));
        }
        text.push_str("endmodule\n");
        text
    }
    let mut file_set = FileSet::default();
    let mut change = Change::new();
    for index in 0..3 {
        let file_id = FileId::from_raw(index as u32);
        file_set.insert(file_id, VfsPath::new_virtual_path(format!("/m{index}.sv")));
        change.add_changed_file(ChangedFile::create(file_id, large_module(index)));
    }
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    let mut host = AnalysisHost::new(Some(2));
    host.apply_change(change);
    let files = [FileId::from_raw(0), FileId::from_raw(1), FileId::from_raw(2)];
    for file in files {
        let _ = host.ctx().file_facts(file);
    }
    let mut change = Change::new();
    let mut edited = large_module(2);
    edited.insert_str(edited.find("endmodule").expect("large_module"), "  wire x;\n");
    change.add_changed_file(ChangedFile::modify(files[2], edited));
    host.apply_change(change);
    let started = Instant::now();
    let _ = host.ctx().file_facts(files[0]);
    print_ms("file_facts.lru_miss", 3, started.elapsed());
    let started = Instant::now();
    let _ = host.ctx().file_facts(files[1]);
    print_ms("file_facts.lru_hit", 3, started.elapsed());
    finish(&mut host);
}

#[test]
#[ignore = "run with --release -- --ignored --nocapture"]
fn first_request_after_body_edit() {
    let files = 256;
    let mut host = workspace_with_modules(files);
    let _ = host.ctx().unit_catalog();

    let mut change = Change::new();
    change.add_changed_file(ChangedFile::modify(FileId::from_raw(0), {
        let mut text = module_text(0);
        text.insert_str(text.find("endmodule").expect("module_text"), "  wire x;\n");
        text
    }));
    host.apply_change(change);

    let started = Instant::now();
    let hover = host
        .make_analysis()
        .hover(FilePosition {
            file_id: FileId::from_raw(0),
            offset: "module ".len().try_into().unwrap(),
        })
        .unwrap();
    print_ms("post_edit.hover", files, started.elapsed());
    assert!(hover.is_some(), "body-only edit must still hover the module name");

    let started = Instant::now();
    let nav = host
        .make_analysis()
        .goto_definition(FilePosition {
            file_id: FileId::from_raw(0),
            offset: "module ".len().try_into().unwrap(),
        })
        .unwrap();
    print_ms("post_edit.goto", files, started.elapsed());
    assert!(nav.is_some(), "body-only edit must still go to the module name");
    finish(&mut host);
}
