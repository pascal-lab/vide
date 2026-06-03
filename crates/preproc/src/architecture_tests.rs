use std::{
    fs,
    path::{Path, PathBuf},
};

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

fn production_source(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
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
fn phase2_boundary_sources_do_not_use_raw_slang_or_fallback_paths() {
    let root = repo_root();
    let checked_sources = [
        root.join("crates/hir/src/base_db/preproc_index.rs"),
        root.join("crates/preproc/src/lib.rs"),
    ];
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

    for path in checked_sources {
        let source = read(&path);
        for pattern in forbidden {
            assert!(
                !source.contains(pattern),
                "{} must not use raw slang, old source-map, or textual fallback pattern `{pattern}`",
                path.display()
            );
        }
    }
}

#[test]
fn macrodb_boundary_does_not_use_slang_syntax_or_old_source_maps() {
    let root = repo_root();
    let macro_db = read(root.join("crates/preproc/src/macro_db.rs"));
    let production_macro_db = production_source(&macro_db);
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

#[test]
fn preproc_trace_model_does_not_use_raw_slang_syntax_types() {
    let root = repo_root();
    let trace = read(root.join("crates/preproc/src/trace.rs"));
    let production_trace = production_source(&trace);
    let forbidden = [
        "slang::",
        "use slang",
        "syntax::slang_ext",
        "slang_ext::",
        "SyntaxNode",
        "SyntaxToken",
        "SyntaxTree",
        "SourceRange",
        "RawSyntax",
    ];

    for pattern in forbidden {
        assert!(
            !production_trace.contains(pattern),
            "PreprocTrace production model must not expose raw slang syntax pattern `{pattern}`"
        );
    }
}

#[test]
fn hir_and_ide_do_not_import_low_level_preproc_trace_constructors() {
    let root = repo_root();
    let forbidden = [
        "preproc::PreprocTrace",
        "preproc::SourceProvenance",
        "PreprocTrace",
        "SourceProvenance",
        "MacroExpansionEvent",
        "ExpandedToken",
        "IncludeEvent",
        "ConditionalEvent",
        "MacroArgument",
        "MacroBody",
        "MacroCall",
    ];

    for dir in ["crates/hir/src", "crates/ide/src"] {
        for path in rust_files(&root.join(dir)) {
            let source = read(&path);
            for pattern in forbidden {
                assert!(
                    !source.contains(pattern),
                    "{} must not directly import low-level preproc trace constructor `{pattern}`",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn slang_adapter_is_the_preproc_trace_extraction_boundary() {
    let root = repo_root();
    let adapter = read(root.join("crates/slang-adapter/src/lib.rs"));
    assert!(adapter.contains("extract_preproc_trace"));
    assert!(adapter.contains("slang binding does not expose expansion trace"));

    let forbidden = ["extract_preproc_trace", "PreprocTraceInput", "PreprocTraceBuffer"];
    for dir in ["crates/hir/src", "crates/ide/src", "crates/preproc/src"] {
        for path in rust_files(&root.join(dir)) {
            if path.file_name().is_some_and(|name| name == "architecture_tests.rs") {
                continue;
            }
            let source = read(&path);
            for pattern in forbidden {
                assert!(
                    !source.contains(pattern),
                    "{} must not bypass slang-adapter trace extraction boundary `{pattern}`",
                    path.display()
                );
            }
        }
    }
}
