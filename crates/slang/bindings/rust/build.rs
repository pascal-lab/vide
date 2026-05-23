use std::{
    env,
    path::{Path, PathBuf},
};

mod sourcegen;

fn main() {
    let debug = cfg!(debug_assertions);
    let cxxbridge_dir = generate_cxx_bridge();
    let install_dir = build_cpp_lib(&cxxbridge_dir, debug);
    setup_linking(&install_dir, debug);
    setup_rerun_triggers();
    generate_rs();
}

fn generate_cxx_bridge() -> PathBuf {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let cxxbridge_dir = out_dir.join("cxxbridge");

    drop(cxx_build::bridges(["bindings/rust/ffi.rs", "bindings/rust/ffi/cxx_sv.rs"]));

    cxxbridge_dir
}

fn build_cpp_lib(cxxbridge_dir: &Path, debug: bool) -> PathBuf {
    let cmake_profile = if debug { "Debug" } else { "Release" };

    // Configure CMake build
    let config = &mut cmake::Config::new(".");
    config
        .env("CMAKE_BUILD_PARALLEL_LEVEL", "16")
        .define("FETCHCONTENT_TRY_FIND_PACKAGE_MODE", "NEVER")
        .define("SLANG_MASTER_PROJECT", "OFF")
        .define("SLANG_INCLUDE_TESTS", "OFF")
        .define("SLANG_INCLUDE_TOOLS", "OFF")
        .define("SLANG_INCLUDE_INSTALL", "ON")
        .define("SLANG_INCLUDE_PYLIB", "OFF")
        .define("SLANG_INCLUDE_RUSTLIB", "ON")
        .define("SLANG_RUST_CXXBRIDGE_DIR", cxxbridge_dir.to_string_lossy().as_ref())
        .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreadedDLL")
        .profile(cmake_profile)
        .define("CMAKE_VERBOSE_MAKEFILE", "ON");

    config.build()
}

fn setup_linking(install_dir: &Path, debug: bool) {
    let lib_dir = install_dir.join("lib");
    let fmt_lib = if debug { "fmtd" } else { "fmt" };
    let mimalloc_lib = if cfg!(target_env = "msvc") {
        if debug { "mimalloc-static-debug" } else { "mimalloc-static" }
    } else {
        if debug { "mimalloc-debug" } else { "mimalloc" }
    };

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static:+whole-archive,-bundle=slang_rust_bridge");
    println!("cargo:rustc-link-lib=static:-bundle=svlang");
    println!("cargo:rustc-link-lib=static:-bundle={}", fmt_lib);
    println!("cargo:rustc-link-lib=static:-bundle={}", mimalloc_lib);
    if cfg!(target_os = "windows") {
        // mimalloc's Windows large-page support pulls in these token APIs.
        println!("cargo:rustc-link-lib=dylib=Advapi32");
    }
}

fn setup_rerun_triggers() {
    let paths = ["CMakeLists.txt", "bindings", "cmake", "external", "include", "scripts", "source"];

    for path in paths {
        println!("cargo:rerun-if-changed={}", path);
    }
}

fn generate_rs() {
    let (all_types, kind_map) = sourcegen::loader::load_types();
    sourcegen::generator::generate_syntax_kind(&kind_map);
    sourcegen::generator::generate_ast_file(&all_types, &kind_map);

    if let Ok(tokens) = sourcegen::loader::load_token_macros() {
        sourcegen::generator::generate_token_macro(tokens);
    }
}
