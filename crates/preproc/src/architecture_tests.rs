use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const PHASE_2_BASE_COMMIT: &str = "17821ff8";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("preproc crate should live under crates/")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref()).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.as_ref().display());
    })
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap_or_else(|err| {
        panic!("failed to read directory {}: {err}", root.display());
    }) {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn git_diff_added_lines(args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|err| panic!("failed to run git diff for architecture gate: {err}"));

    assert!(
        output.status.success(),
        "git diff for architecture gate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("git diff output should be UTF-8")
}

#[test]
fn hir_preproc_index_is_migration_only_reexport() {
    let path = repo_root().join("crates/hir/src/base_db/preproc_index.rs");
    let source = read(path);

    assert!(source.contains("migration_only"));
    assert!(source.contains("pub use preproc::directive_index::*;"));
    assert!(
        !source.contains("MacroDb"),
        "hir preproc_index must not expose MacroDb as a parallel HIR boundary"
    );
}

#[test]
fn ide_does_not_depend_on_preproc_directly() {
    let manifest = read(repo_root().join("crates/ide/Cargo.toml"));

    assert!(
        !manifest.contains("preproc"),
        "IDE macro semantics should go through an explicit semantic facade, not direct preproc dependency"
    );
}

#[test]
fn hir_and_ide_do_not_import_macrodb_or_slang_adapter() {
    let root = repo_root();
    let forbidden = [
        "slang_adapter::",
        "use slang_adapter",
        "preproc::MacroDb",
        "MacroDb",
        "macro_definition_at(",
        "macro_references(",
    ];

    for dir in ["crates/hir/src", "crates/ide/src"] {
        for path in rust_files(&root.join(dir)) {
            let source = read(&path);
            for pattern in forbidden {
                assert!(
                    !source.contains(pattern),
                    "{} must not contain forbidden Phase 2 boundary pattern `{pattern}`",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn phase2_hir_ide_diff_does_not_add_raw_slang_or_fallback_paths() {
    let diff = git_diff_added_lines(&[
        "diff",
        "--unified=0",
        PHASE_2_BASE_COMMIT,
        "--",
        "crates/hir/Cargo.toml",
        "crates/hir/src",
        "crates/ide/Cargo.toml",
        "crates/ide/src",
    ]);
    let forbidden = [
        "slang::",
        "use slang",
        "pub use slang",
        "syntax::slang_ext",
        "slang_ext::",
        "source_map",
        "SourceMap",
        "Source2Def",
        "source_to_def",
        "_with_source_map",
        "fallback",
        "full_text",
        "full text",
        "regex::",
        "Regex",
    ];

    for line in diff.lines().filter(|line| line.starts_with('+') && !line.starts_with("+++")) {
        for pattern in forbidden {
            assert!(
                !line.contains(pattern),
                "Phase 2 must not add HIR/IDE raw slang, old source-map, or textual fallback path `{pattern}` in diff line: {line}"
            );
        }
    }
}

#[test]
fn macrodb_boundary_does_not_use_slang_syntax_or_old_source_maps() {
    let root = repo_root();
    let macro_db = read(root.join("crates/preproc/src/macro_db.rs"));
    let production_macro_db = macro_db.split("#[cfg(test)]").next().unwrap_or(&macro_db);
    let forbidden = [
        "slang::",
        "syntax::",
        "SyntaxTree",
        "source_map",
        "SourceMap",
        "Source2Def",
        "source_to_def",
        "_with_source_map",
        "regex::",
        "Regex",
    ];

    for pattern in forbidden {
        assert!(
            !production_macro_db.contains(pattern),
            "MacroDb must not depend on raw slang/syntax or old source-map fallback pattern `{pattern}`"
        );
    }
}
