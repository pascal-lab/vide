---
title: Advanced Troubleshooting
description: Troubleshoot local VSIX packages, custom servers, file watching, logs, and profiling by symptom.
---

This page handles issues beyond normal feature usage, such as local VSIX packages, replacement servers, file watching, server logs, and diagnostics profiling. For ordinary diagnostics, navigation, formatting, or Qihe usage problems, start from the related page in [Features](../../user-guide/features/).

If you cannot yet confirm whether the language server started, run through [Server Self-Check](../check-server/) first. Command, status bar, and output channel names are listed in [Commands, Status, and Logs](../../user-guide/commands-status-logs/).

## Start from the Symptom

| Symptom | Start here | Common cause |
| --- | --- | --- |
| The status bar shows a language-server error | `Vide Language Server` output channel | VSIX platform mismatch, missing custom command, invalid working directory |
| A local VSIX cannot find the server | "Local VSIX Cannot Find the Server" on this page | The extension was compiled without bundling the server into the VSIX |
| A custom server works in a terminal but fails in the extension | "Custom Server Startup Fails" on this page | `vide.server.cwd`, argument arrays, or VS Code process PATH differ from the terminal |
| File changes do not refresh | "File Changes Do Not Trigger Refresh" on this page | File watcher events are missing, or files are excluded |
| You need internal logs or performance data | "Server Logs" and "Diagnostics Profiling" on this page | Additional startup arguments or profiling artifacts are needed |

## Local VSIX Cannot Find the Server

The extension looks for `server/vide.exe` or `server/vide` under its own installation directory. During local debugging, running only `npm run compile` builds the extension JavaScript, but does not build the server or copy it into the extension directory.

To create a local VSIX that includes the server, run this under `editors/vscode`:

```powershell
npm run package:debug
```

If you only want an installed extension to use a locally built server, configure a custom server path instead:

```json
{
  "vide.server.command": "D:\\Proj\\vide\\target\\release\\vide.exe"
}
```

After saving, accept the `Restart` prompt or run `Vide: Restart Language Server`. See [Build from Source](../build-from-source/) for the full build flow.

## Custom Server Startup Fails

First confirm the command that the extension is actually using. Open the `Vide Language Server` output channel and find `Server command`, `Server args`, and `Working directory`.

Then check:

- `vide.server.command` uses an absolute path.
- The command can run `--version` in a terminal.
- `vide.server.args` and `vide.server.additionalArgs` are arrays of strings.
- If `vide.server.cwd` is set, it points to an existing directory.
- After changing `vide.server.command`, `vide.server.args`, `vide.server.additionalArgs`, `vide.server.cwd`, or `vide.trace.server`, restart the language server.

Example:

```json
{
  "vide.server.command": "D:\\tools\\vide\\vide.exe",
  "vide.server.args": [],
  "vide.server.cwd": "D:\\work\\chip",
  "vide.server.additionalArgs": ["--log", "debug"]
}
```

See [Server settings](../../user-guide/vscode-settings/#server) for the full field reference.

## Status Bar Shows a Project Configuration Error

A status bar error does not always come from language-server startup. Click the `Vide` status item, open the output, and separate the source:

- `Bundled Vide Language Server binary not found`, `Unsupported platform-architecture combination`, or `Failed to start language server`: continue checking the VSIX or custom server.
- `failed to load workspace`, `manifest ...`, or `vide.toml` related errors: this is a project configuration error.

Project configuration errors should be fixed in the workspace root `vide.toml`. See [Configure the First Project](../../user-guide/first-project/) or [Project Configuration Reference](../../user-guide/project-configuration/).

## File Changes Do Not Trigger Refresh

The default `vide.files.watcher` is `client`, so Vide prefers VS Code watched-file notifications. If the client does not support dynamic watched files, Vide falls back to the server-side watcher.

If project file changes do not trigger a refresh, temporarily switch to the server watcher:

```json
{
  "vide.files.watcher": "server"
}
```

`vide.files.excludeDirs` only accepts workspace-relative directories and does not support globs. Prefer `sources` and `exclude` in `vide.toml` for project file selection. If you also want to reduce VS Code watcher events, configure VS Code's `files.watcherExclude` separately.

## Open More Detailed Server Logs

If the language server starts but you need internal logs, add `--log` and `--log_file` through `vide.server.additionalArgs`, then restart the language server:

```json
{
  "vide.server.additionalArgs": ["--log", "debug", "--log_file", "D:\\work\\vide-server.log"]
}
```

If the server itself still cannot start, avoid complex log arguments first. Use [Server Self-Check](../check-server/) to confirm the server path, platform, and `--version`.

## Diagnostics Profiling Artifacts Are Missing

`Vide: Profile Diagnostics` starts an isolated temporary language server and does not reuse the current editor session. The artifact directory, trace, summary, and flamegraph paths are written to the `Vide Profiling` output channel.

If no artifacts appear:

- Confirm that the current workspace or current file can be analyzed normally.
- Open the `Vide Profiling` output channel and check the temporary server startup error.
- The temporary server still uses the current `vide.server.command`, `vide.server.args`, and related settings; custom server errors also affect profiling.

Artifact formats are described in [Commands, Status, and Logs](../../user-guide/commands-status-logs/#advanced-diagnostics-profiling-artifacts).
