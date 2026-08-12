//! Integration tests with real-world .core file fixtures.

use fusesoc_model::{load_core_file, normalize, project, resolve, vlnv};
use utils::paths::AbsPathBuf;

fn fixture_dir(name: &str) -> AbsPathBuf {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("tests/fixtures").join(name);
    utils::paths::abs_path_buf_from_path_buf(path.to_path_buf()).unwrap()
}

#[test]
fn loads_darkriscv_core_file() {
    let dir = fixture_dir("darkriscv");
    let core_path = dir.join("darkriscv.core");
    let core = load_core_file(&core_path).unwrap();
    assert_eq!(core.name, "darklife:darkriscv:darkriscv:1.0");
    assert!(core.filesets.contains_key("rtl"));
    assert!(core.filesets.contains_key("tb"));
    assert!(core.targets.contains_key("default"));
    assert!(core.targets.contains_key("sim"));
}

#[test]
fn darkriscv_default_target_expands_correctly() {
    let dir = fixture_dir("darkriscv");
    let core_path = dir.join("darkriscv.core");

    // Load core.
    let mut core = load_core_file(&core_path).unwrap();
    normalize::normalize_core(&mut core);

    // Verify toplevel normalization (scalar → list).
    let default_target = core.targets.get("default").unwrap();
    assert_eq!(default_target.top_modules(), vec!["darksocv"]);

    // Verify fileset expansion.
    let rtl_fs = core.filesets.get("rtl").unwrap();
    assert_eq!(rtl_fs.files.len(), 2);
    assert_eq!(rtl_fs.files[0].path(), "rtl/darksocv.v");

    // Verify include file detection.
    let include_entry = &rtl_fs.files[1];
    assert_eq!(include_entry.path(), "rtl/config.vh");
    let attrs = include_entry.attributes().unwrap();
    assert!(attrs.is_include_file);
    assert_eq!(attrs.include_path.as_deref(), Some("rtl"));

    // Verify file_type inheritance.
    assert_eq!(
        normalize::effective_file_type(&rtl_fs.files[0], rtl_fs),
        Some("verilogSource".to_string())
    );
}

#[test]
fn darkriscv_resolves_to_resolved_project() {
    let dir = fixture_dir("darkriscv");

    // Build index and resolve.
    let (index, parse_errors) = resolve::CoreIndex::from_roots(std::slice::from_ref(&dir));
    assert!(parse_errors.is_empty(), "{parse_errors:?}");

    let top_vlnv = vlnv::Vlnv::parse("darklife:darkriscv:darkriscv:1.0").unwrap();
    let graph = index.resolve(&top_vlnv, "default");
    assert!(graph.errors.is_empty(), "{:?}", graph.errors);
    assert_eq!(graph.cores.len(), 1);

    // Expand into resolved project.
    let resolved = project::expand(&graph, "default");
    assert_eq!(resolved.top_modules, vec!["darksocv"]);

    // Should have 2 source files (darksocv.v + config.vh).
    assert_eq!(resolved.files.len(), 2);

    // darksocv.v is a regular source file.
    let darksocv = resolved.files.iter().find(|f| {
        f.path
            .file_name()
            .is_some_and(|n| n == "darksocv.v")
    });
    assert!(darksocv.is_some(), "darksocv.v should be in resolved files");
    assert!(!darksocv.unwrap().is_include_file);

    // config.vh is an include file.
    let config = resolved.files.iter().find(|f| {
        f.path
            .file_name()
            .is_some_and(|n| n == "config.vh")
    });
    assert!(config.is_some(), "config.vh should be in resolved files");
    assert!(config.unwrap().is_include_file);

    // Include dir should be rtl/.
    assert!(
        resolved
            .include_dirs
            .iter()
            .any(|d| d.file_name().is_some_and(|n| n == "rtl")),
        "include_dirs should contain rtl/, got {:?}",
        resolved.include_dirs
    );
}

#[test]
fn darkriscv_sim_target_has_different_toplevel() {
    let dir = fixture_dir("darkriscv");

    let (index, parse_errors) = resolve::CoreIndex::from_roots(std::slice::from_ref(&dir));
    assert!(parse_errors.is_empty(), "{parse_errors:?}");

    let top_vlnv = vlnv::Vlnv::parse("darklife:darkriscv:darkriscv:1.0").unwrap();
    let graph = index.resolve(&top_vlnv, "sim");
    assert!(graph.errors.is_empty(), "{:?}", graph.errors);

    let resolved = project::expand(&graph, "sim");
    assert_eq!(resolved.top_modules, vec!["darksimv"]);
    // sim target includes both rtl and tb filesets → 3 files.
    assert_eq!(resolved.files.len(), 3);
}