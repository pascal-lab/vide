use std::{
    env, fs,
    path::{Path, PathBuf},
};

const SLANG_SOURCE_DIR: &str = "../../third_party/slang";
const SLANG_SYS_SOURCE_DIR: &str = "./src";
const SCRIPTS_DIR: &str = "./scripts";
/// Build directory from cargo target directory.
const BUILD_DIR: &str = "slang-sys";

fn main() {
    // Prepare environment
    let slang_dir = env_detection::find_slang_dir();
    let source_dir = PathBuf::from(SLANG_SYS_SOURCE_DIR);
    let scripts_dir = PathBuf::from(SCRIPTS_DIR);
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let debug = cfg!(debug_assertions);

    // Build
    generate_rust_defs(&slang_dir, &out_dir);
    let install_dir = build_slang(&slang_dir, debug);
    build_cxx_bridge(&slang_dir, &install_dir);

    // Setup cargo configuration
    setup_linking(&install_dir, debug);
    setup_rerun_triggers(&slang_dir, &source_dir, &scripts_dir);
}

mod env_detection {
    use std::{env, path::PathBuf};

    use super::{BUILD_DIR, SLANG_SOURCE_DIR};

    pub fn find_slang_dir() -> PathBuf {
        let slang_source_dir = PathBuf::from(SLANG_SOURCE_DIR);
        if !slang_source_dir.join("CMakeLists.txt").is_file() {
            panic!(
                "SLANG_SOURCE_DIR is set to {}, but that directory does not contain CMakeLists.txt!\nYou may need to run \"git submodule update --init\" to initialize the submodule",
                slang_source_dir.display()
            );
        };
        slang_source_dir
    }

    pub fn target_linker_flags() -> Option<String> {
        env::var("TARGET_LDFLAGS").ok().filter(|flags| !flags.trim().is_empty())
    }

    pub fn target_is_msvc() -> bool {
        env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    }

    pub fn target_is_windows() -> bool {
        env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    }

    pub fn build_dir() -> PathBuf {
        let workspace_target_dir =
            env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| {
                PathBuf::from(
                    env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"),
                )
                .join("../..")
                .join("target")
            });
        workspace_target_dir.join(BUILD_DIR)
    }
}

fn generate_rust_defs(_slang_dir: &Path, _out_dir: &Path) {
    // TODO: Support generating Rust definitions for the Slang API.
}

fn build_slang(slang_dir: &Path, debug: bool) -> PathBuf {
    let cmake_profile = if debug { "Debug" } else { "Release" };
    let cmake_out_dir = env_detection::build_dir().join(cmake_profile.to_ascii_lowercase());
    let emscripten = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten");

    // Configure CMake build
    let config = &mut cmake::Config::new(slang_dir);
    config
        .out_dir(cmake_out_dir)
        .define("FETCHCONTENT_TRY_FIND_PACKAGE_MODE", "NEVER")
        .define("CMAKE_EXPORT_COMPILE_COMMANDS", "ON")
        .profile(cmake_profile)
        .define("CMAKE_VERBOSE_MAKEFILE", "ON");
    if let Ok(jobs) = env::var("NUM_JOBS") {
        config.env("CMAKE_BUILD_PARALLEL_LEVEL", jobs);
    }
    // Build flags <https://sv-lang.com/building.html#build-options>
    config
        .define("SLANG_MASTER_PROJECT", "OFF")
        .define("SLANG_INCLUDE_TESTS", "OFF")
        .define("SLANG_INCLUDE_TOOLS", "OFF")
        .define("SLANG_INCLUDE_INSTALL", "ON")
        .define("SLANG_INCLUDE_PYLIB", "OFF")
        // TODO: We may need to support mimalloc in the future. But we need to figure out the
        // linking issue first. The default build option of slang will generate mimalloc object file
        // rather thant the static library :(.
        .define("SLANG_USE_MIMALLOC", "OFF")
        // .define("SLANG_RUST_CXXBRIDGE_DIR", cxxbridge_dir.to_string_lossy().as_ref())
        .define("CMAKE_INSTALL_LIBDIR", "lib");

    if emscripten {
        config
            .define("CMAKE_TRY_COMPILE_TARGET_TYPE", "STATIC_LIBRARY")
            .define("CMAKE_CXX_FLAGS", "-fwasm-exceptions -include cstdlib")
            .define("CMAKE_CXX_FLAGS_RELEASE", "-O2 -DNDEBUG")
            .define("CMAKE_C_FLAGS_RELEASE", "-O2 -DNDEBUG");
        if let Ok(toolchain_file) = env::var("EMSCRIPTEN_CMAKE_TOOLCHAIN_FILE") {
            config.define("CMAKE_TOOLCHAIN_FILE", toolchain_file);
        }
    } else {
        if env_detection::target_is_msvc() {
            config.define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreadedDLL");
        } else {
            config.cxxflag("-include").cxxflag("cstdlib");
        }
    }

    // TODO: Port cmake sccache launcher

    if let Some(linker_flags) = env_detection::target_linker_flags() {
        config
            .define("CMAKE_EXE_LINKER_FLAGS", linker_flags.as_str())
            .define("CMAKE_SHARED_LINKER_FLAGS", linker_flags.as_str())
            .define("CMAKE_MODULE_LINKER_FLAGS", linker_flags.as_str());
    }

    if !emscripten && !debug && env_detection::target_is_msvc() {
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

fn build_cxx_bridge(slang_dir: &Path, install_dir: &Path) {
    // Setup clangd include directory for cxx crate
    let cxx_header = PathBuf::from(
        env::var_os("DEP_CXXBRIDGE1_HEADER")
            .expect("DEP_CXXBRIDGE1_HEADER is not set; the cxx crate should expose its C++ header"),
    );
    let cxx_include_dir = cxx_header
        .parent()
        .expect("DEP_CXXBRIDGE1_HEADER should point to a header under an include directory")
        .to_path_buf();
    let clangd_include_dir = env_detection::build_dir().join("cxx").join("include");
    fs::create_dir_all(&clangd_include_dir).expect("failed to create clangd cxx include directory");
    fs::copy(cxx_header, clangd_include_dir.join("cxx.h"))
        .expect("failed to copy cxx.h for clangd");
    // Build cxx bridge
    cxx_build::bridge("src/ffi.rs")
        .file("src/wrapper.cpp")
        .include("src")
        .include(cxx_include_dir)
        .include(install_dir.join("include"))
        .include(slang_dir.join("external"))
        .define("SLANG_BOOST_SINGLE_HEADER", None)
        .define("SLANG_STATIC_DEFINE", None)
        .flag_if_supported("-std=c++20")
        .compile("slang_sys_bridge");
}

fn setup_linking(install_dir: &Path, debug: bool) {
    let lib_dir = install_dir.join("lib");
    let emscripten = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten");
    let fmt_lib = if debug { "fmtd" } else { "fmt" };

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static:-bundle=svlang");
    println!("cargo:rustc-link-lib=static:-bundle={}", fmt_lib);
    if !emscripten && env_detection::target_is_windows() {
        // mimalloc's Windows large-page support pulls in these token APIs.
        println!("cargo:rustc-link-lib=dylib=Advapi32");
    }
}

fn setup_rerun_triggers(slang_dir: &Path, source_dir: &Path, scripts_dir: &Path) {
    let watch = [
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"),
        slang_dir.to_string_lossy().to_string(),
        source_dir.join("ffi.rs").to_string_lossy().to_string(),
        source_dir.join("wrapper.cpp").to_string_lossy().to_string(),
        source_dir.join("wrapper.h").to_string_lossy().to_string(),
        scripts_dir.to_string_lossy().to_string(),
    ];

    for path in watch {
        println!("cargo:rerun-if-changed={}", path);
    }
}
