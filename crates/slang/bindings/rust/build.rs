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
    let emscripten = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten");

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
        .profile(cmake_profile)
        .define("CMAKE_VERBOSE_MAKEFILE", "ON");

    if !emscripten {
        config.define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreadedDLL");
    }

    if emscripten {
        config
            .define("SLANG_USE_MIMALLOC", "OFF")
            .define("CMAKE_TRY_COMPILE_TARGET_TYPE", "STATIC_LIBRARY")
            .define("CMAKE_CXX_FLAGS", "-fwasm-exceptions -include cstdlib")
            .define("CMAKE_CXX_FLAGS_RELEASE", "-O2 -DNDEBUG")
            .define("CMAKE_C_FLAGS_RELEASE", "-O2 -DNDEBUG");
        if let Ok(toolchain_file) = env::var("EMSCRIPTEN_CMAKE_TOOLCHAIN_FILE") {
            config.define("CMAKE_TOOLCHAIN_FILE", toolchain_file);
        }
    }

    if !emscripten && !debug && cfg!(target_env = "msvc") {
        // cmake-rs still sets config-specific MSVC flags for Visual Studio
        // generators to preserve /MD or /MT. That replaces CMake's built-in
        // Release defaults, while cmake-rs has already filtered optimization
        // args out of Cargo's compiler flags. Restore the optimized Release
        // settings explicitly until cmake-rs can rely on
        // CMAKE_MSVC_RUNTIME_LIBRARY for this path.
        config
            .define("CMAKE_C_FLAGS_RELEASE", "/O2 /Ob2 /DNDEBUG")
            .define("CMAKE_CXX_FLAGS_RELEASE", "/O2 /Ob2 /DNDEBUG");
    }

    config.build()
}

fn setup_linking(install_dir: &Path, debug: bool) {
    let lib_dir = install_dir.join("lib");
    let emscripten = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten");
    let fmt_lib = if debug { "fmtd" } else { "fmt" };

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static:+whole-archive,-bundle=slang_rust_bridge");
    println!("cargo:rustc-link-lib=static:-bundle=svlang");
    println!("cargo:rustc-link-lib=static:-bundle={}", fmt_lib);
    if !emscripten {
        let mimalloc_lib = if cfg!(target_env = "msvc") {
            if debug { "mimalloc-static-debug" } else { "mimalloc-static" }
        } else {
            if debug { "mimalloc-debug" } else { "mimalloc" }
        };
        println!("cargo:rustc-link-lib=static:-bundle={}", mimalloc_lib);
    }
    if !emscripten && cfg!(target_os = "windows") {
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
