//! Synthetic incrementality benches. Run with:
//! `cargo test -p ide --release --lib incrementality_benches -- --ignored --nocapture`

use std::time::Instant;

use base_db::{change::Change, source_root::SourceRoot};
use vfs::{ChangedFile, FileId, FileSet, VfsPath};

use crate::{FilePosition, analysis_host::AnalysisHost};

fn workspace_with_modules(n: usize) -> AnalysisHost {
    let mut file_set = FileSet::default();
    let mut change = Change::new();
    for index in 0..n {
        let file_id = FileId::from_raw(index as u32);
        file_set.insert(file_id, VfsPath::new_virtual_path(format!("/m{index}.sv")));
        change.add_changed_file(ChangedFile::create(
            file_id,
            format!("module m{index};\nendmodule\n"),
        ));
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
        let host = workspace_with_modules(files);
        let started = Instant::now();
        let graph = host.ctx().design_graph();
        print_ms("design_graph.fold", files, started.elapsed());
        assert_eq!(graph.node_count(), files);
    }
}

#[test]
#[ignore = "run with --release -- --ignored --nocapture"]
fn first_request_after_body_edit() {
    let files = 256;
    let mut host = workspace_with_modules(files);
    let _ = host.ctx().design_graph();

    let mut change = Change::new();
    change.add_changed_file(ChangedFile::modify(
        FileId::from_raw(0),
        "module m0;\n  wire x;\nendmodule\n",
    ));
    host.apply_change(change);

    let started = Instant::now();
    let hover = host
        .make_analysis()
        .hover(FilePosition { file_id: FileId::from_raw(0), offset: "module ".len().try_into().unwrap() })
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
}
