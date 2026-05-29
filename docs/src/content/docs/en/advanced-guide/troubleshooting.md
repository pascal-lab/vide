---
title: Troubleshooting and Bug Reports
description: Report Vide issues and troubleshoot common startup and refresh problems.
---

Vide can still fail in some cases.

Some failures are product issues. In that case, please report them through [GitHub Issues](https://github.com/pascal-lab/vide/issues) and attach a reproducible example when possible. See [Report a Bug](#report-a-bug) for the information to include.
Some failures come from workspace or extension configuration. In that case, start with [Common Cases and Responses](#common-cases-and-responses).

## Report a Bug

If a feature behaves incorrectly, please open a report in [GitHub Issues](https://github.com/pascal-lab/vide/issues) and include, when possible:

- a minimal example that triggers the problem
- the expected behavior and the actual behavior
- your platform, VS Code version, and the extension/server versions shown by `Vide: Show Server Version`
- stable reproduction steps when the problem is reproducible

Start by running `Vide: Show Server Version`, then `Vide: Show Language Server Output`, and attach the relevant content from the `Vide Language Server` output channel to the issue.

If the `Vide Language Server` output channel still does not provide enough information, or the problem only appears in a longer-running flow, enable a more detailed file log. Add `--log` and `--log_file` through `vide.server.additionalArgs`, then restart the language server:

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
  first confirm that the installed VSIX matches the current platform. If you installed a locally packaged VSIX, confirm that it was built with `npm run package:*` or `npm run package:debug`. Those commands bundle the language server binary into the VSIX; `npm run compile` only builds the extension frontend, so the installed extension will not contain the server.
- `Failed to start language server`, missing custom command, or permission failure:
  continue with "The Extension or Custom Server Cannot Start" below.
- The status bar only mentions `vide.toml`, `manifest`, or `failed to load workspace`:
  this is usually not a startup problem. Go back to the workspace-root `vide.toml`. See [Configure the First Project](../../user-guide/first-project/) and [Project Configuration Reference](../../user-guide/project-configuration/).

### The Extension or Custom Server Cannot Start

Use this section when the VS Code extension cannot start the bundled server, or when a custom `vide.server.command` cannot start.

First confirm the exact command the VS Code extension uses to start the language server. Run `Vide: Show Status`, `Vide: Show Language Server Output`, and `Vide: Show Server Version`, then record:

- `Platform`
- `Server command`
- `Server args`
- `Working directory`

If `Vide: Show Server Version` also fails, the server command, working directory, or base arguments currently used by the extension are not runnable yet.

Validate the same command directly in a terminal:

```powershell
vide --version
```

Use the `Server command` and arguments shown above. If you configured a custom server, also check:

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

Use this section when adding, deleting, or modifying project files does not make Vide reload the project.

The default `vide.files.watcher` is `client`. This means the VS Code extension prefers to forward file-change events from VS Code to the Vide language server. If the current editor environment does not support dynamic watched files, Vide automatically falls back to the language server process's own file watcher.

If project file changes do not trigger a refresh, temporarily switch to the language server process's own watcher:

```json
{
  "vide.files.watcher": "server"
}
```

`vide.files.excludeDirs` is a VS Code extension setting that tells Vide which workspace-relative directories to ignore. It only accepts directory names and does not support glob syntax such as `**`.

Use `sources` and `exclude` in `vide.toml` to choose which files belong to the project. If you also want VS Code itself to watch fewer large directories, configure VS Code's `files.watcherExclude` separately.
