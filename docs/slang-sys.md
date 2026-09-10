# slang-sys

`slang-sys` is the Rust FFI boundary for the Slang C++ library. It is intended to replace the old `slang` crate as the FFI interface used
by Vide crates.

Unlike old `slang` crate, `slang-sys` doesn't vendored or edit the upstream slang repository. Instead, it uses `git submodule` to reference the upstream slang and provides rust bindings inside the crate, this will not only avoid introducing bugs into slang code base but also make it easier to keep up with the latest slang version. You can find the submodule in `third_party/slang` directory.

## slang integration
Since slang is written in C++, and use some trick to generate code when compiling itself, we need to use some special procedure to build slang library and linked it with `slang-sys` crate. You can find the details in [build.rs](../crates/slang-sys/build.rs).

Here is a brief summary of the build procedure:
1. Generate rust definitions from slang IDL files (for example, [syntax.txt](../third_party/slang/scripts/syntax.txt)). The build scripts will call generator scripts (under `../crates/slang-sys/scripts/` directory), and emit rust code into `OUT_DIR` directory. When you navigate around the slang-sys source code, you may jump to the `target/` directory occasionally, that's where the generated rust code is located. :)

2. Build slang library. The build script will call `cmake` to build the slang library, and install it into `OUT_DIR` directory.

3. Generate cxx bridge code. Since we rely on `cxx` crate to provide C++ FFI support, we need to generate cxx bridge code (C++) for the FFI functions and types defined in the `**/ffi.rs` files.

4. Linking all the things together.

## Development
### Setup
To develop `slang-sys`, you need to follow the steps below:

1. Since slang-sys need slang source code to build, you need to initialize the submodule first:
```bash
git submodule update --init
```

2. You need to make sure you have the full C++ toolchain installed and is available in your `PATH`, including:
    - clang/clang++ (MSVC on windows)
    - cmake
    - ninja
    - python3
    - rust toolchain (YOU MUST HAVE IT, RIGHT?)

3. For better developer experience, we recommend you to install `clangd`. If you have installed it, it should read `.clangd` and `.clang-format` files correctly and automatically provide right IDE features in the `.cpp/.h` files inside this crate and `third_party/slang` codebase after you build the slang-sys crates.

### Edit slang-sys
If you want to edit slang-sys, for example, add new FFI functions, here are some tips for you:

1. Conventionally, we put FFI definitions and C++ wrapper files inside the corresponding module directory, for example, `diagnostic/ffi.rs`, `diagnostic/wrapper.h`, `diagnostic/wrapper.cpp` are all related to diagnostic module. It will be better to follow this convention since it will improve the readability for FFI code.

2. If you add more FFI file / C++ wrapper file / C++ header file, you need to add them into the `FFI_FILES` / `WRAPPER_HEADERS` / `WRAPPER_SOURCES` in the [build.rs](../crates/slang-sys/build.rs) file, otherwise they will not be compiled and linked correctly.

### Update slang version
Theoretically, you can just update the submodule version, everything should work. **Don't forget to update the crate version of slang-sys, it should be aligned with the slang version.**
