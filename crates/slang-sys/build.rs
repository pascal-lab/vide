use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const SLANG_SOURCE_DIR: &str = "../../third_party/slang";
const SLANG_SYS_SOURCE_DIR: &str = "./src";
const SCRIPTS_DIR: &str = "./scripts";
const USE_SCCACHE_CMAKE_ENV: &str = "VIDE_USE_SCCACHE_CMAKE";
const SCCACHE_PATH_ENV: &str = "SCCACHE_PATH";
const VERBOSE_BUILD_ENV: &str = "VIDE_SLANG_BUILD_VERBOSE";
/// Build directory root from cargo target directory.
///
/// The concrete CMake directory is qualified by target and profile below.
/// CMake caches compiler and toolchain detection, so sharing one directory
/// across target triples is incorrect even when the Rust profile is equal.
const BUILD_DIR: &str = "slang-sys";
/// FFI files from src directory.
const FFI_FILES: &[&str] = &["compilation/ffi.rs", "diagnostic/ffi.rs", "syntax/ffi.rs"];
/// CPP wrapper headers from src directory.
const WRAPPER_HEADERS: &[&str] =
    &["wrapper.h", "compilation/wrapper.h", "diagnostic/wrapper.h", "syntax/wrapper.h"];
/// CPP wrapper files from src directory.
const WRAPPER_FILES: &[&str] =
    &["compilation/wrapper.cpp", "diagnostic/wrapper.cpp", "syntax/wrapper.cpp"];

fn main() {
    let total_started = Instant::now();

    // Prepare environment
    let slang_dir = env_detection::find_slang_dir();
    let source_dir = PathBuf::from(SLANG_SYS_SOURCE_DIR);
    let scripts_dir = PathBuf::from(SCRIPTS_DIR);
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let debug = cfg!(debug_assertions);
    setup_rerun_triggers(&slang_dir, &source_dir, &scripts_dir);

    // Build
    let started = Instant::now();
    generate_rust_defs(&slang_dir, &out_dir, &scripts_dir);
    log_phase("generate-rust-defs", started.elapsed());

    let started = Instant::now();
    let install_dir = build_slang(&slang_dir, debug);
    log_phase("build-slang", started.elapsed());

    let started = Instant::now();
    build_cxx_bridge(&slang_dir, &install_dir, &source_dir, &out_dir, debug);
    log_phase("build-cxx-bridge", started.elapsed());

    // Setup cargo configuration
    setup_linking(&install_dir, debug);
    log_phase("total", total_started.elapsed());
}

fn log_phase(phase: &str, elapsed: Duration) {
    eprintln!("slang-sys-build phase={phase} elapsed_ms={}", elapsed.as_millis());
}

mod env_detection {
    use std::{env, ffi::OsString, path::PathBuf};

    use super::{
        BUILD_DIR, SCCACHE_PATH_ENV, SLANG_SOURCE_DIR, USE_SCCACHE_CMAKE_ENV, VERBOSE_BUILD_ENV,
    };

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
        let target = env::var("TARGET").expect("TARGET is not set");
        build_root().join(target)
    }

    pub fn build_root() -> PathBuf {
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

    pub fn cmake_cache_key() -> String {
        let mut entries = env::vars_os()
            .filter(|(name, _)| {
                let name = name.to_string_lossy();
                name == "CC"
                    || name == "CXX"
                    || name == "CMAKE_GENERATOR"
                    || name == "CMAKE_TOOLCHAIN_FILE"
                    || name == "EMSCRIPTEN_CMAKE_TOOLCHAIN_FILE"
                    || name == "TARGET_LDFLAGS"
                    || name.starts_with("CC_")
                    || name.starts_with("CXX_")
                    || name.starts_with("CMAKE_GENERATOR_")
                    || name.starts_with("CMAKE_TOOLCHAIN_FILE_")
                    || name.starts_with("HOST_CC")
                    || name.starts_with("HOST_CXX")
                    || name.starts_with("HOST_CMAKE_GENERATOR")
                    || name.starts_with("HOST_CMAKE_TOOLCHAIN_FILE")
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        // Stable FNV-1a: this key is a cache namespace, not a security boundary.
        let mut hash = 0xcbf29ce484222325u64;
        for (name, value) in entries {
            for byte in
                name.as_encoded_bytes().iter().chain([0].iter()).chain(value.as_encoded_bytes())
            {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        format!("{hash:016x}")
    }

    pub fn find_python() -> PathBuf {
        env::var_os("PYTHON").map(PathBuf::from).unwrap_or_else(|| "python3".into())
    }

    pub fn verbose_build() -> bool {
        env::var_os(VERBOSE_BUILD_ENV)
            .is_some_and(|value| parse_enabled_flag(VERBOSE_BUILD_ENV, &value))
    }

    pub fn cmake_compiler_launcher() -> Option<PathBuf> {
        if let Some(value) = env::var_os(USE_SCCACHE_CMAKE_ENV) {
            if !parse_enabled_flag(USE_SCCACHE_CMAKE_ENV, &value) {
                return None;
            }

            let requested =
                env::var_os(SCCACHE_PATH_ENV).unwrap_or_else(|| OsString::from("sccache"));
            return Some(resolve_launcher(&requested));
        }

        let wrapper = env::var_os("RUSTC_WRAPPER").filter(|wrapper| !wrapper.is_empty())?;
        let wrapper_path = PathBuf::from(&wrapper);
        let stem = wrapper_path.file_stem()?.to_str()?.to_owned();
        ["sccache", "cachepot", "buildcache", "kache"]
            .contains(&stem.as_str())
            .then(|| resolve_launcher(&wrapper))
    }

    fn resolve_launcher(requested: &OsString) -> PathBuf {
        which::which(requested).unwrap_or_else(|error| {
            panic!(
                "CMake compiler caching requested launcher {requested:?}, but it could not be \
                 executed: {error}. Install it or set {SCCACHE_PATH_ENV} to its executable path"
            )
        })
    }

    fn parse_enabled_flag(name: &str, value: &OsString) -> bool {
        match value.to_string_lossy().trim().to_ascii_lowercase().as_str() {
            "" | "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => panic!("{name} must be one of 1/true/yes/on or 0/false/no/off, got {value:?}"),
        }
    }

    pub fn cargo_manifest_dir() -> PathBuf {
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"))
    }
}

fn generate_rust_defs(slang_dir: &Path, out_dir: &Path, scripts_dir: &Path) {
    let python = env_detection::find_python();
    let slang_scripts_dir = slang_dir.join("scripts");
    let generators = [
        (
            scripts_dir.join("generate_syntax_kind.py"),
            slang_scripts_dir.join("syntax.txt"),
            out_dir.join("syntax_kind.rs"),
            Vec::<PathBuf>::new(),
        ),
        (
            scripts_dir.join("generate_token_kind.py"),
            slang_scripts_dir.join("tokenkinds.txt"),
            out_dir.join("token_kind.rs"),
            Vec::<PathBuf>::new(),
        ),
        (
            scripts_dir.join("generate_trivia_kind.py"),
            slang_scripts_dir.join("triviakinds.txt"),
            out_dir.join("trivia_kind.rs"),
            Vec::<PathBuf>::new(),
        ),
        (
            scripts_dir.join("generate_diagnostic.py"),
            slang_scripts_dir.join("diagnostics.txt"),
            out_dir.join("diagnostic.rs"),
            vec![
                "--diagnostics-header".into(),
                slang_dir.join("include/slang/diagnostics/Diagnostics.h"),
            ],
        ),
        (
            scripts_dir.join("generate_ast.py"),
            slang_scripts_dir.join("syntax.txt"),
            out_dir.join("ast.rs"),
            Vec::<PathBuf>::new(),
        ),
    ];

    let mut children = generators.map(|(generator, input, output, extra_args)| {
        let mut command = Command::new(&python);
        command.arg(generator).arg("--input").arg(input);
        for arg in extra_args {
            command.arg(arg);
        }
        command
            .arg("--out")
            .arg(output)
            .spawn()
            .expect("failed to run slang-sys Rust definition generator")
    });

    for child in &mut children {
        let status = child.wait().expect("failed to wait for slang-sys Rust definition generator");
        if !status.success() {
            panic!("slang-sys Rust definition generator failed with status {status}");
        }
    }
}

fn build_slang(slang_dir: &Path, debug: bool) -> PathBuf {
    let cmake_profile = if debug { "Debug" } else { "Release" };
    let cmake_out_dir = env_detection::build_dir()
        .join(env_detection::cmake_cache_key())
        .join(cmake_profile.to_ascii_lowercase());
    let emscripten = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten");
    let compiler_launcher = env_detection::cmake_compiler_launcher();
    let verbose = env_detection::verbose_build();
    let rustc_wrapper = env::var_os("RUSTC_WRAPPER")
        .filter(|wrapper| !wrapper.is_empty())
        .map(|wrapper| wrapper.to_string_lossy().into_owned())
        .unwrap_or_else(|| "disabled".into());

    eprintln!(
        "slang-sys-build target={} cmake_profile={cmake_profile} build_dir={} generator={} \
         cmake_launcher={} bridge_wrapper={} verbose={verbose}",
        env::var("TARGET").as_deref().unwrap_or("<unknown>"),
        cmake_out_dir.display(),
        env::var("CMAKE_GENERATOR").as_deref().unwrap_or("<cmake-default>"),
        compiler_launcher
            .as_deref()
            .map_or_else(|| "disabled".into(), |path| path.display().to_string()),
        rustc_wrapper,
    );
    if compiler_launcher.is_some() && env::var_os("SCCACHE_BASEDIRS").is_none() {
        eprintln!(
            "slang-sys-build note=SCCACHE_BASEDIRS-is-unset; cross-worktree cache hits may be reduced"
        );
    }

    // Configure CMake build
    let config = &mut cmake::Config::new(slang_dir);
    config
        .out_dir(cmake_out_dir)
        .define("FETCHCONTENT_TRY_FIND_PACKAGE_MODE", "NEVER")
        .define("CMAKE_EXPORT_COMPILE_COMMANDS", "ON")
        .profile(cmake_profile)
        .define("CMAKE_VERBOSE_MAKEFILE", if verbose { "ON" } else { "OFF" });
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

    let launcher = compiler_launcher.as_deref().map_or_else(|| "".into(), Path::to_string_lossy);
    config
        .define("CMAKE_C_COMPILER_LAUNCHER", launcher.as_ref())
        .define("CMAKE_CXX_COMPILER_LAUNCHER", launcher.as_ref());

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

fn build_cxx_bridge(
    slang_dir: &Path,
    install_dir: &Path,
    source_dir: &Path,
    out_dir: &Path,
    debug: bool,
) {
    // Setup clangd include directory for cxx crate
    let cxx_header = PathBuf::from(
        env::var_os("DEP_CXXBRIDGE1_HEADER")
            .expect("DEP_CXXBRIDGE1_HEADER is not set; the cxx crate should expose its C++ header"),
    );
    let cxx_include_dir = cxx_header
        .parent()
        .expect("DEP_CXXBRIDGE1_HEADER should point to a header under an include directory")
        .to_path_buf();
    let clangd_include_dir = env_detection::build_root().join("clangd").join("include");
    fs::create_dir_all(&clangd_include_dir).expect("failed to create clangd cxx include directory");
    fs::copy(cxx_header, clangd_include_dir.join("cxx.h"))
        .expect("failed to copy cxx.h for clangd");
    // Build cxx bridge
    let ffi_files = FFI_FILES.iter().map(|f| PathBuf::from(source_dir).join(f));
    let wrapper_files = WRAPPER_FILES.iter().map(|f| PathBuf::from(source_dir).join(f));
    let wrapper_header_dirs = WRAPPER_HEADERS
        .iter()
        .map(|f| PathBuf::from(source_dir).join(f).parent().unwrap().to_path_buf());
    let mut build = cxx_build::bridges(ffi_files);
    build
        .files(wrapper_files)
        .includes(wrapper_header_dirs)
        .include(cxx_include_dir)
        .include(install_dir.join("include"))
        .include(slang_dir.join("external"))
        .define("SLANG_BOOST_SINGLE_HEADER", None)
        .define("SLANG_STATIC_DEFINE", None);

    if env_detection::target_is_msvc() {
        build.flag_if_supported("/std:c++20");
    } else {
        build.flag_if_supported("-std=c++20");
    }
    if debug {
        build.define("SLANG_DEBUG", None);
    }
    build.compile("slang_sys_bridge");
    copy_headers_recursively(&out_dir.join("cxxbridge/include"), &clangd_include_dir);
    copy_headers_recursively(&install_dir.join("include"), &clangd_include_dir);
}

fn copy_headers_recursively(from: &Path, to: &Path) {
    if !from.is_dir() {
        return;
    }

    for entry in fs::read_dir(from).expect("failed to read cxxbridge include directory") {
        let entry = entry.expect("failed to read cxxbridge include entry");
        let source = entry.path();
        let destination = to.join(entry.file_name());
        if source.is_dir() {
            fs::create_dir_all(&destination).expect("failed to create clangd include directory");
            copy_headers_recursively(&source, &destination);
        } else {
            fs::copy(&source, &destination).expect("failed to copy cxxbridge header for clangd");
        }
    }
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
    let mut watch = vec![
        env_detection::cargo_manifest_dir().join("build.rs").to_string_lossy().to_string(),
        slang_dir.to_string_lossy().to_string(),
        scripts_dir.to_string_lossy().to_string(),
    ];
    let ffi_files =
        FFI_FILES.iter().map(|f| PathBuf::from(source_dir).join(f).to_string_lossy().to_string());
    let wrapper_files = WRAPPER_FILES
        .iter()
        .map(|f| PathBuf::from(source_dir).join(f).to_string_lossy().to_string());
    let wrapper_headers = WRAPPER_HEADERS
        .iter()
        .map(|f| PathBuf::from(source_dir).join(f).to_string_lossy().to_string());
    watch.extend(ffi_files);
    watch.extend(wrapper_files);
    watch.extend(wrapper_headers);

    for path in watch {
        println!("cargo:rerun-if-changed={}", path);
    }
    for name in [USE_SCCACHE_CMAKE_ENV, SCCACHE_PATH_ENV, VERBOSE_BUILD_ENV] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-env-changed=RUSTC_WRAPPER");
}
