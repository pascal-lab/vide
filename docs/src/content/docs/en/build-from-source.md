---
title: Build from Source
description: Build the Vizsla server, VS Code extension, and local VSIX packages from source.
---

This page is for users who need local development, debugging, or VSIX packaging.

## Build the Rust Server

Run this from the repository root:

```powershell
cargo build
```

Release build:

```powershell
cargo build --release
```

Verify the version:

```powershell
.\target\release\vizsla.exe --version
```

On non-Windows platforms:

```powershell
./target/release/vizsla --version
```

If you only want the VS Code extension to use the locally built server, configure:

```json
{
  "vizsla.server.command": "D:\\Proj\\vizsla\\target\\release\\vizsla.exe"
}
```

## Build the VS Code Extension

Enter the extension directory:

```powershell
cd editors\vscode
npm install
npm run compile
```

`npm run compile` cleans previous output, runs TypeScript type checking, and bundles with esbuild to generate `dist/extension.js`.

## Package a VSIX

Run this under `editors\vscode`:

```powershell
npm run package
```

This command:

1. Compiles the extension.
2. Runs `cargo build --release` for the current host platform.
3. Copies `target/release/vizsla` or `vizsla.exe` into the extension's `server/<target>` directory.
4. Temporarily places the server binary in the runtime `server` directory.
5. Calls `vsce package --target <target>` to generate `vizsla-vscode-<target>.vsix`.
6. Cleans up the temporary runtime binary after packaging.

You can also specify a target:

```powershell
npm run package:win32-x64
npm run package:linux-x64
```

Cross-platform packaging does not automatically cross-compile the Rust server. The script requires the target platform server binary to already exist under `editors/vscode/server/<target>/`, or you should package on a matching native runner.

## Install a Local VSIX

After packaging, run:

```powershell
npm run install-extension
```

The install script looks for `vizsla-vscode-*.vsix` in the current directory. If multiple VSIX files exist and no filter is specified, it installs the most recently modified one.

You can also run:

```powershell
code --install-extension .\vizsla-vscode-win32-x64.vsix
```

This command requires `code` to be available on `PATH`.
