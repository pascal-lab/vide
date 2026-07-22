---
title: VS Code Advanced Installation
description: Build Vide from source, package local VSIX files, or install prerelease VS Code builds.
---

## Introduction

In most cases, start with [VS Code Installation](../../user-guide/vscode-installation/) and install the stable Marketplace extension in one step. Use this page instead when:

- you want to build and install Vide from source: see [Build and Install from Source](#build-and-install-from-source).
- you want to try a prerelease build: see [Install a Prerelease Build](#install-a-prerelease-build).

<a id="build-and-install-from-source"></a>

## Build and Install from Source

This section is for users who want to install Vide from source, modify the codebase, or package their own VSIX files.

### Prerequisites

If you only need to build the Vide language server, install:

- Rust nightly-2026-05-24
  - This repository pins that toolchain; `cargo` comes with it
- CMake 3.20 through 3.29
- Python 3
- A C++20-capable compiler
  - Linux: GCC 11 or newer, or Clang 16 or newer
  - macOS: Xcode 15 or newer
  - Windows: the latest Visual Studio 2022 Build Tools, with the `Desktop development with C++` workload

If you also want to build the VS Code extension or package a VSIX, additionally install:

- Node.js 22.x
- npm
  - The npm bundled with Node.js 22 is sufficient

If you also want to build the Playground WASM version, additionally install:

- Emscripten SDK 5.0.2
- `ninja`
- The Rust `wasm32-unknown-emscripten` target
  - `website/playground/scripts/build-vide-wasm.mjs` automatically runs `rustup target add wasm32-unknown-emscripten`

If you want to package a local VSIX, you need both sets of tools above. If you also want to build the Playground WASM version, you need all three sets.

You do not need to install a separate system `slang` command first; Vide uses the vendored `slang` sources in this repository during the build.

### Build the Vide Language Server

Vide's core is a Rust language server. Semantic editor features such as navigation, completion, hover, rename, and diagnostics are primarily provided by that server; the VS Code extension starts it, communicates with it, and connects its results to the editor UI.

To build that language server, run this from the repository root:

```powershell
cargo build
```

Release build:

```powershell
cargo build --release
```

For ordinary local builds, you do not need to set `VIDE_BUILD_METADATA`. In beta, nightly, or release workflows, CI and release scripts set it when needed so that `vide --version` carries the extra build marker.

If you already installed the VS Code extension, you can point it at the server you just built. The plain `cargo build` command above produces a debug binary, so point VS Code at `target/debug`:

```json
{
  "vide.server.command": "D:/Proj/vizsla/target/debug/vide.exe"
}
```

If you built with `cargo build --release`, use `D:/Proj/vizsla/target/release/vide.exe` instead.

After saving, VS Code prompts you to `Restart`; accept that, then run `Vide: Show Server Version` to verify the binary used by the extension. If you also need startup arguments or a working directory, see the full [VS Code Settings Reference](../../user-guide/vscode-settings/#server).

You can also continue and build a complete VS Code extension package.

### Build the VS Code Extension

Enter the VS Code extension directory and compile it:

```powershell
cd editors/vscode
npm ci
npm run compile
```

`npm run compile` does three things:

1. Removes `out` and `dist`, then runs the TypeScript typecheck.
2. Bundles `src/extension.ts` into `dist/extension.js` with esbuild.
3. Does not copy the Speedscope static assets used by diagnostics profiling by default.

### Package the VS Code Extension as a VSIX

If you want a local debug build or a VSIX with debug binaries, run this under `editors/vscode`:

```powershell
npm run package:vsix:debug
```

This command:

1. Compiles the extension, so it is fine if you did not run `npm run compile` manually first.
2. Copies the Speedscope static assets required by diagnostics profiling and enables the `profile-trace` server feature.
3. Uses `cargo xtask vscode prepare-server` to prepare a debug server for the current host platform.
4. Copies `target/debug/vide` or `vide.exe` into the extension's `server/<target>` directory.
5. Temporarily stages the server binary in the runtime `server` directory.
6. Calls `vsce package --target <target>` to generate `vide-vscode-<target>-debug.vsix`.
7. Cleans up the temporary runtime binary after packaging.

If you want a release VSIX for a specific platform, run one or more of these commands:

```powershell
npm run package:vsix -- --target linux-x64
npm run package:vsix -- --target linux-arm64
npm run package:vsix -- --target win32-x64
npm run package:vsix -- --target darwin-arm64
npm run package:vsix -- --target alpine-x64 --server=prebuilt
npm run package:vsix -- --target alpine-arm64 --server=prebuilt
```

These scripts compile the extension, prepare a release server binary for the target platform, and generate `vide-vscode-<target>.vsix`. Release packages do not enable profile trace, and they do not include Speedscope static assets or the profiling command by default. The current release workflow only covers those targets: glibc Linux, Windows x64, macOS arm64, and Alpine/musl x64/arm64.
Those are also the VSIX targets currently built by CI. Other platforms are not current packaging targets.

All packaging commands above need to prepare the language server binary for the target platform first. `editors/vscode/scripts/package.ts` calls `cargo xtask vscode prepare-server`, and the reusable server build rules live under `cargo xtask server build`:

- When the target matches the current host platform, xtask runs `cargo build` for the selected profile and copies the result.
- Alpine targets are built by the release artifact workflow as prebuilt musl language servers. To package an Alpine VSIX locally, first place the matching `vide` binary under `editors/vscode/server/<target>/`, then pass `--server=prebuilt`.
- Other non-host targets are not automatically cross-compiled; the matching `vide` or `vide.exe` must already exist under `editors/vscode/server/<target>/`, or you should package on a matching native runner.

### Install the VS Code Extension

After packaging, run:

```powershell
npm run install-extension
```

The install script looks for `vide-vscode-*.vsix` in the current directory. If multiple VSIX files exist and no filter is specified, it installs the most recently modified one. You can pass a filename fragment to select a specific VSIX:

```powershell
npm run install-extension -- win32-x64-debug
```

You can also run:

```powershell
code --install-extension ./vide-vscode-win32-x64-debug.vsix
```

This command requires `code` to be available on `PATH`.

<a id="install-a-prerelease-build"></a>

## Install a Prerelease Build

Prerelease installation lets you try upcoming Vide beta features. Before installing, first obtain a `.vsix` package.

### Choose an Installation Channel

You can download a `.vsix` file and install it manually. Choose the source based on the version you want:

| Version | Source | Use when |
| --- | --- | --- |
| Stable | [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=pascal-lab.vide-ide), or the latest non-prerelease entry in [GitHub Releases](https://github.com/pascal-lab/vide/releases) | Daily use and offline installation |
| Beta | A prerelease entry in [GitHub Releases](https://github.com/pascal-lab/vide/releases) | You want to try the next version early |
| Nightly dev package | Artifacts from [GitHub Actions CI](https://github.com/pascal-lab/vide/actions/workflows/ci.yml), named like `vide-vscode-dev-<target>-<commit>` | You need to verify a specific commit or a latest fix |

VSIX packages are platform-specific. Current release and CI artifacts cover these targets:

- `alpine-x64`
- `alpine-arm64`
- `darwin-arm64`
- `linux-arm64`
- `linux-x64`
- `win32-x64`

### Install a VSIX

After you have a `.vsix` file, install it from the VS Code command palette:

1. Open the command palette.
2. Run `Extensions: Install from VSIX...`.
3. Select the `vide-vscode-*.vsix` file for your platform.

You can also drag the `.vsix` file directly into the VS Code Extensions view. When installation succeeds, VS Code shows a confirmation notification in the lower-right corner.

You can also install from the command line:

```powershell
code --install-extension ./vide-vscode-win32-x64.vsix
```

If the status bar reports an error after installation, use [Troubleshooting and Bug Reports](../troubleshooting/) to confirm the server path and platform package.
