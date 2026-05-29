---
title: Troubleshooting and Bug Reports
description: Report Vide issues and troubleshoot common startup and refresh problems.
---

Vide can still fail in some cases.

Some failures are product issues. In that case, please report them through [GitHub Issues](https://github.com/pascal-lab/vide/issues) and attach a reproducible example when possible.
Some failures come from workspace or extension configuration. In that case, start with the common cases on this page.

For normal feature usage questions, go back to the related page under [Features](../../user-guide/features/). Command names, status bar meanings, and output channel names are listed in [VS Code Status Bar, Command Palette, and Logs](../../user-guide/commands-status-logs/).

## Report a Bug

If a feature behaves incorrectly, please open a report in [GitHub Issues](https://github.com/pascal-lab/vide/issues) and include, when possible:

- a minimal example that triggers the problem
- the expected behavior and the actual behavior
- your platform, VS Code version, and the extension/server versions shown by `Vide: Show Server Version`
- stable reproduction steps when the problem is reproducible

Start by running `Vide: Show Server Version`, then `Vide: Show Language Server Output`, and attach the relevant content from the `Vide Language Server` output channel to the issue.

If the output is still not enough, or the problem only appears in a longer-running flow, enable a more detailed file log. Add `--log` and `--log_file` through `vide.server.additionalArgs`, then restart the language server:

```json
{
  "vide.server.additionalArgs": ["--log", "debug", "--log_file", "D:/work/vide-server.log"]
}
```

If the server itself cannot start, go straight to "The Extension or Custom Server Cannot Start" below.

## Common Cases and Responses

### The Status Bar Shows a Language Server Error

Open the `Vide Language Server` output channel first. Focus on the last error, and on whether you see:

```text
[INFO] Language server started successfully
```

Common branches are:

- `Bundled Vide Language Server binary not found` or `Unsupported platform-architecture combination`:
  first confirm that the installed VSIX matches the current platform. For local packaging, only `npm run package:*` or `npm run package:debug` bundles the server into the VSIX; `npm run compile` only builds the extension frontend.
- `Failed to start language server`, missing custom command, or permission failure:
  continue with "The Extension or Custom Server Cannot Start" below.
- The status bar only mentions `vide.toml`, `manifest`, or `failed to load workspace`:
  this is usually not a startup problem. Go back to the workspace-root `vide.toml`. See [Configure the First Project](../../user-guide/first-project/) and [Project Configuration Reference](../../user-guide/project-configuration/).

See [Build and Install from Source](../advanced-installation/#build-and-install-from-source) for the full local packaging and installation flow.

### The Extension or Custom Server Cannot Start

First confirm the command actually used by the extension. Run `Vide: Show Status`, `Vide: Show Language Server Output`, and `Vide: Show Server Version`, then record:

- `Platform`
- `Server command`
- `Server args`
- `Working directory`

If `Vide: Show Server Version` also fails, the server command, working directory, or base arguments currently used by the extension are not runnable yet.

You can also validate the same binary directly in a terminal:

```powershell
vide --version
```

Windows custom-server example:

```powershell
D:/tools/vide/vide.exe --version
```

If you configured a custom server, also check:

- `vide.server.command` uses an absolute path
- the command can run `--version` successfully in a terminal
- `vide.server.args` and `vide.server.additionalArgs` are arrays of strings
- if `vide.server.cwd` is set, it points to an existing directory
- after changing `vide.server.command`, `vide.server.args`, `vide.server.additionalArgs`, `vide.server.cwd`, or `vide.trace.server`, restart the language server

Example:

```json
{
  "vide.server.command": "D:/tools/vide/vide.exe",
  "vide.server.args": [],
  "vide.server.cwd": "D:/work/chip",
  "vide.server.additionalArgs": ["--log", "debug"]
}
```

See [Server settings](../../user-guide/vscode-settings/#server) for the full field reference.

### File Changes Do Not Trigger Refresh

The default `vide.files.watcher` is `client`, so Vide prefers VS Code watched-file notifications. If the client does not support dynamic watched files, Vide falls back to the server-side watcher.

If project file changes do not trigger a refresh, temporarily switch to the server watcher:

```json
{
  "vide.files.watcher": "server"
}
```

`vide.files.excludeDirs` only accepts workspace-relative directories and does not support globs. Prefer `sources` and `exclude` in `vide.toml` for project file selection. If you also want to reduce VS Code watcher events, configure VS Code's `files.watcherExclude` separately.
