---
title: Build from Source
description: Build the Vide server, VS Code extension, and local VSIX packages from source.
---

This page is for users who need local server builds, extension startup debugging, or VSIX packaging. After building, use [Server Self-Check Flow](../check-server/) to verify the server launch.

## Prerequisites

When building Vide from source, `cargo build` compiles the vendored
`crates/slang` tree through a Rust build script, so the build needs a C++
environment that can compile slang:

- Rust toolchain and Cargo.
- CMake 3.20 or newer.
- A Python interpreter for slang's CMake configuration step.
- A C++20-capable C++ compiler. On Windows, install Visual Studio 2022 Build
  Tools with the "Desktop development with C++" workload; on Linux/macOS, use a
  recent GCC or Clang toolchain. slang requires at least GCC 10-level C++20
  support.
- Node.js and npm for building the VS Code extension and packaging VSIX files.

You do not need to install a system-level `slang` command first. Vide uses the
vendored slang sources in this repository, and server builds or VSIX packaging
compile them together with the Rust server.

## Build the Rust Server

Run this from the repository root:

```powershell
cargo build
```

Release build:

```powershell
cargo build --release
```

By default, `vide --version` only includes the package version and build
profile; it does not read the current Git commit automatically. For beta,
nightly, or internal builds that need an extra marker, set
`VIDE_BUILD_METADATA` explicitly:

```powershell
$env:VIDE_BUILD_METADATA = "abc1234.20260529T120000Z"
cargo build --release
```

Verify the version:

```powershell
.\target\release\vide.exe --version
```

On non-Windows platforms:

```powershell
./target/release/vide --version
```

If you only want the VS Code extension to use the locally built server, configure:

```json
{
  "vide.server.command": "D:\\Proj\\vide\\target\\release\\vide.exe"
}
```

After saving, VS Code prompts you to `Restart`; accept it, then use `Vide: Show Server Version` to verify the binary used by the extension.

## Build the VS Code Extension

Enter the extension directory:

```powershell
cd editors\vscode
npm ci
npm run compile
```

`npm run compile` only builds the extension itself: it removes `out` and
`dist`, runs TypeScript type checking, bundles `src/extension.ts` to
`dist/extension.js` with esbuild, and copies the speedscope static assets needed
by the profiling view into `dist/speedscope`. This step does not build or copy
the Vide server binary.

## Package a VSIX

For a local debugging VSIX, run this under `editors\vscode`:

```powershell
npm run package:debug
```

This command:

1. Compiles the extension.
2. Runs `cargo build` for the current host platform.
3. Copies `target/debug/vide` or `vide.exe` into the extension's `server/<target>` directory.
4. Temporarily places the server binary in the runtime `server` directory.
5. Calls `vsce package --target <target>` to generate `vide-vscode-<target>-debug.vsix`.
6. Cleans up the temporary runtime binary after packaging.

The release workflow currently produces release VSIX packages for these targets:

```powershell
npm run package:linux-x64
npm run package:linux-arm64
npm run package:win32-x64
npm run package:darwin-arm64
npm run package:alpine-x64
npm run package:alpine-arm64
```

These scripts compile the extension, prepare a release server binary for the
target platform, and generate `vide-vscode-<target>.vsix`. The current release
workflow only covers the targets above: glibc Linux, Windows x64, macOS arm64,
and Alpine/musl x64 and arm64.

`package.json` also keeps `package:win32-arm64` and `package:darwin-x64` entry
points for manual verification or future release-matrix expansion. The current
release flow does not publish official VSIX artifacts for those two targets.

Server binary preparation is controlled by `editors/vscode/scripts/package.ts`:

- When the target matches the current host platform, the script runs
  `cargo build --release` and copies the result.
- Alpine targets are built in musl containers in CI. The local script adds the
  matching Rust musl target, but still needs a working musl cross-compilation
  environment.
- Other non-host targets are not automatically cross-compiled; the matching
  `vide` or `vide.exe` must already exist under
  `editors/vscode/server/<target>/`, or you should package on a matching native
  runner.

## Install a Local VSIX

After packaging, run:

```powershell
npm run install-extension
```

The install script looks for `vide-vscode-*.vsix` in the current directory. If multiple VSIX files exist and no filter is specified, it installs the most recently modified one.
You can pass a filename fragment to select a specific VSIX:

```powershell
npm run install-extension -- win32-x64-debug
```

You can also run:

```powershell
code --install-extension .\vide-vscode-win32-x64-debug.vsix
```

This command requires `code` to be available on `PATH`.
