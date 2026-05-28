---
title: Advanced Installation
description: Install local VSIX packages, choose release channels, or configure a custom Vide language server.
---

In most cases, follow [VS Code Installation](../../user-guide/vscode-installation/) to install the stable Marketplace extension. This page is for offline installation, local validation, prerelease packages, and custom servers.

## Choose an Installation Channel

You can download a `.vsix` file and install it manually. Choose the source based on the version you want:

| Version | Source | Use when |
| --- | --- | --- |
| Stable | [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=pascal-lab.vide), or the latest non-prerelease entry in [GitHub Releases](https://github.com/pascal-lab/vide/releases) | Daily use and offline installation |
| Beta | A prerelease entry in [GitHub Releases](https://github.com/pascal-lab/vide/releases) | You want to try the next version early |
| Nightly dev package | Artifacts from [GitHub Actions CI](https://github.com/pascal-lab/vide/actions/workflows/ci.yml), named like `vide-vscode-dev-<target>-<commit>` | You need to verify a specific commit or a latest fix |

VSIX packages are platform-specific. Current release and CI artifacts cover these targets:

- `alpine-arm64`
- `alpine-x64`
- `darwin-arm64`
- `linux-arm64`
- `linux-x64`
- `win32-x64`

## Install a VSIX

After you have a `.vsix` file, install it from the VS Code command palette:

1. Open the command palette.
2. Run `Extensions: Install from VSIX...`.
3. Select the `vide-vscode-*.vsix` file for your platform.

You can also install from the command line:

```powershell
code --install-extension .\vide-vscode-win32-x64.vsix
```

If the status bar reports an error after installation, use [Server Self-Check](../check-server/) to confirm the server path and platform package.

## Configure a Custom Server

The extension uses the bundled language server by default. Configure `vide.server.command` only when you need to replace the server binary or debug startup arguments.

These cases are good reasons to configure a custom server:

- You built `vide` from source and want the extension to use your local binary.
- You are debugging server startup arguments or logs.
- The bundled server is missing or does not match the current platform.
- You need to temporarily test a specific server version.

Use an absolute path:

```json
{
  "vide.server.command": "D:\\tools\\vide\\vide.exe",
  "vide.server.args": [],
  "vide.server.cwd": "D:\\path\\to\\workspace",
  "vide.server.additionalArgs": ["--log", "debug"]
}
```

Both `vide.server.args` and `vide.server.additionalArgs` must be arrays of strings. When the extension starts the server, it passes `server.args` first and then appends `server.additionalArgs`. See the full [VS Code Settings Reference](../../user-guide/vscode-settings/#server).
