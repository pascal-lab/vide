---
title: Advanced Troubleshooting
description: Debug local VSIX packages, bundled servers, custom servers, file watching, logs, and profiling.
---

This page keeps advanced startup, logging, and debugging issues only. For normal stale diagnostics, failed navigation, formatting failures, or Qihe run failures, start from the related page in [Daily Use](../../user-guide/daily-use/).

For startup checks, use [Server Self-Check](../check-server/). Command, status bar, and output channel entry points are in the [Operations Reference](../commands-status-logs/).

## Local VSIX Cannot Find the Server

The extension looks for `server/vide.exe` or `server/vide` under its own installation directory. When preparing or debugging a local VSIX, running only `npm run compile` does not create a bundled server or copy it into the extension directory.

Package a debug VSIX under `editors/vscode`:

```powershell
npm run package:debug
```

Or configure a local server directly:

```json
{
  "vide.server.command": "D:\\Proj\\vide\\target\\release\\vide.exe"
}
```

After saving, accept the `Restart` prompt or run `Vide: Restart Language Server`.

## Custom Command, Args, or cwd Startup Fails

Check these points:

- `vide.server.command` uses an absolute path and can run `--version` in a terminal.
- `vide.server.args` and `vide.server.additionalArgs` are arrays of strings.
- If `vide.server.cwd` is set, it points to an existing directory.
- After changing `vide.server.command`, `vide.server.args`, `vide.server.additionalArgs`, `vide.server.cwd`, or `vide.trace.server`, accept the extension's `Restart` prompt.

Example:

```json
{
  "vide.server.command": "D:\\tools\\vide\\vide.exe",
  "vide.server.args": [],
  "vide.server.cwd": "D:\\work\\chip",
  "vide.server.additionalArgs": ["--log", "debug"]
}
```

See [Server settings](../vscode-settings/#server) for the full field reference.

## Status Bar Shows a Startup Error

Click the `Vide` status item to open the status menu, then choose output, or run `Vide: Show Language Server Output`. Focus on these output lines:

- `Bundled Vide Language Server binary not found`
- `Unsupported platform-architecture combination`
- `Failed to start language server`
- `Server command`
- `Server args`
- `Working directory`

If the error comes from project configuration, fix `vide.toml` at the workspace root using [Project Configuration](../../user-guide/project-configuration/). If the error comes from server launch, continue checking the custom server or VSIX package on this page.

## File Changes Do Not Trigger Refresh

The default `vide.files.watcher` is `client`, so Vide prefers VS Code watched-file notifications. If the client does not support dynamic watched files, Vide falls back to the server-side watcher.

If project file changes do not trigger a refresh:

```json
{
  "vide.files.watcher": "server"
}
```

`vide.files.excludeDirs` only accepts workspace-relative directories and does not support globs. Prefer the project manifest's `sources` / `exclude` globs for file selection. If you also want to reduce VS Code watcher events, configure VS Code's `files.watcherExclude` separately.

## Need More Detailed Server Logs

If the process starts but you need server-side logs, add `--log` and `--log_file` through `vide.server.additionalArgs`, then restart the language server:

```json
{
  "vide.server.additionalArgs": ["--log", "debug", "--log_file", "D:\\work\\vide-server.log"]
}
```

If startup itself fails, avoid extra arguments first; use [Server Self-Check](../check-server/) to confirm the server path, platform, and `--version`.

## Diagnostics Profiling Artifacts Are Missing

`Vide: Profile Diagnostics` starts an isolated temporary language server and does not reuse the current editor session. The artifact directory, trace, summary, and flamegraph paths are written to the `Vide Profiling` output channel.

If no artifacts appear:

- Confirm that the current workspace or current file can be analyzed normally.
- Open the `Vide Profiling` output channel and check the temporary server startup error.
- The temporary server still uses the current `vide.server.command`, `vide.server.args`, and related settings; custom server errors also affect profiling.

Artifact formats are described in [Operations Reference](../commands-status-logs/#advanced-diagnostics-profiling-artifacts).
