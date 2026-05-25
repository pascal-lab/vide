---
title: Installation
description: Install Vizsla from the VS Code extension marketplace, a VSIX package, or a custom server.
---

## Install from the VS Code Extension

Most users only need to install the VS Code extension. On startup, the extension first uses the `vizsla` server bundled with the extension, so you do not need to install Rust separately or start an LSP process manually.

The extension display name is `Vizsla` and the extension ID is `vizsla.vizsla-lsp`. If it is already available from your extension sources, install it directly from the VS Code Extensions view.

After installation, continue with [Quick Start](./quick-start/): open the directory that contains your RTL source code in VS Code. You do not need to write a project manifest first. When a workspace contains Verilog/SystemVerilog source files but no `vizsla.toml` or legacy `vizsla_config.toml`, the extension prompts you to create the default configuration. See [First Project](./first-project/) for what that default means and when a manifest becomes useful.

## Offline or Local VSIX Installation

After you have a `.vsix` file, you can install it from the VS Code command palette:

1. Open the command palette.
2. Run `Extensions: Install from VSIX...`.
3. Select the `vizsla-vscode-*.vsix` file for your platform.

You can also install from the command line:

```powershell
code --install-extension .\vizsla-vscode-win32-x64.vsix
```

VSIX packages are platform-specific. The current packaging script supports these targets:

- `alpine-arm64`
- `alpine-x64`
- `darwin-arm64`
- `darwin-x64`
- `linux-arm64`
- `linux-x64`
- `win32-arm64`
- `win32-x64`

## When to Configure a Custom Server

Do not configure `vizsla.server.command` by default. The extension looks for the bundled server under its own installation directory.

These cases are good reasons to configure a custom server:

- You built `vizsla` from source and want the extension to use your local binary.
- You are debugging server startup arguments or logs.
- The bundled server is missing or does not match the current platform.
- You need to temporarily test a specific server version.

Use an absolute path:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.args": [],
  "vizsla.server.cwd": "D:\\path\\to\\workspace",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

Both `vizsla.server.args` and `vizsla.server.additionalArgs` must be arrays of strings. When the extension starts the server, it passes `server.args` first and then appends `server.additionalArgs`.
