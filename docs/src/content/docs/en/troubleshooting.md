---
title: Troubleshooting by Symptom
description: Troubleshoot status bar, startup, Qihe, diagnostics, project scanning, and file watching symptoms.
---

This page starts from symptoms. To confirm whether the server can launch, use [Server Self-Check Flow](./check-server.md). For command, status item, and output channel entry points, use the [operations reference](./commands-status-logs.md).

## `Vizsla` Status Bar Shows Error or Warning

Click the `Vizsla` status item to open the status menu. Project configuration errors appear at the top of that menu; you can also choose `Show Output` or run `Vizsla: Show Language Server Output`.

Focus on these output lines:

- `Bundled Vizsla Language Server binary not found`
- `Unsupported platform-architecture combination`
- `Failed to start language server`
- `Server command`
- `Server args`
- `Working directory`

If the error comes from project configuration, open or fix the project manifest at the workspace root first. The recommended file name is `vizsla.toml`; the legacy `vizsla_config.toml` name is still supported but deprecated.

## Bundled Server Not Found

The extension looks for `server/vizsla.exe` or `server/vizsla` under its own installation directory. During local development, running only `npm run compile` does not create a bundled server.

Package a debug VSIX under `editors/vscode`:

```powershell
npm run package:debug
```

Or configure a local server directly:

```json
{
  "vizsla.server.command": "D:\\Proj\\vizsla\\target\\release\\vizsla.exe"
}
```

After saving, accept the `Restart` prompt.

## Custom command, args, or cwd Startup Fails

Check these points:

- `vizsla.server.command` uses an absolute path and can run `--version` in a terminal.
- `vizsla.server.args` and `vizsla.server.additionalArgs` are arrays of strings.
- If `vizsla.server.cwd` is set, it points to an existing directory.
- After changing `vizsla.server.command`, `vizsla.server.args`, `vizsla.server.additionalArgs`, `vizsla.server.cwd`, or `vizsla.trace.server`, accept the extension's `Restart` prompt.

Example:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.args": [],
  "vizsla.server.cwd": "D:\\work\\chip",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

## Qihe Command Is Unavailable or the Button Does Nothing

`Vizsla: Run Qihe Analysis` is available only for local Verilog/SystemVerilog files. The target must be a `file:` URI whose extension is `.v`, `.vh`, `.sv`, `.svh`, or `.svi`.

If the Qihe process cannot start:

- Confirm that `vizsla.qihe.command` is an executable name or absolute path, not a project directory.
- If it is `qihe`, confirm that it is on the `PATH` seen by the VS Code process.
- On Windows, VS Code launched from the desktop may see a different `PATH` than your terminal; use an absolute path when unsure.

## Qihe Analysis Fails

While Qihe runs, a separate `Qihe` status item appears. After a failure, clicking that status item opens the `Vizsla Qihe` output channel; the `Show Qihe Output` action in the error notification opens the same channel.

In `Vizsla Qihe`, check the target file, Qihe compile/run arguments, Qihe output, and final failure details. Qihe arguments are derived from the current project manifest by default; projects that already manage those arguments through scripts can disable automatic derivation and configure compile/run arguments explicitly in [VS Code Settings](./vscode-settings.md#qihe).

## Diagnostics Are Too Frequent or Stale

The default `vizsla.diagnostics.update` is `onSave`, so diagnostics refresh when you save. Keep this default for large projects.

If you need diagnostics while editing:

```json
{
  "vizsla.diagnostics.update": "onType"
}
```

If diagnostics do not update, save the file first, then run `Vizsla: Reload Project Configuration`. If they still do not update, run `Vizsla: Restart Language Server` and check for project configuration errors.

## Instance Ports or Parameters Report Errors

If the `Problems` panel reports instance connection or parameter problems, place the cursor near the instance and open the lightbulb menu. Vizsla currently supports these cases:

- Missing port connections: use `Fill connections`.
- Parameters without values: use `Fill parameters`.
- Mixed ordered and named port connections: use `Convert ordered port connections to named connections`, or `Remove empty port connections` when the problem is an extra empty connection.
- Mixed ordered and named parameter assignments: use `Convert ordered parameter assignments to named assignments`.
- A `.port` shorthand missing explicit `()`: use `Add explicit empty port connection`.
- An instance with no port list: use `Add empty instance port list`.

If these actions are missing, confirm that Vizsla can resolve the target module first. A direct check is `Go to Definition` on the instance module name. If that does not jump, fix `sources`, `include_dirs`, `defines`, or `libraries` first. Vizsla currently does not provide a dedicated automatic fix for unresolved modules, missing includes, or unresolved imports.

## You Want to Hide or Downgrade a Diagnostic Type

Place the cursor on the diagnostic and open the lightbulb menu. For slang diagnostics with an identifiable code, Vizsla provides quick fixes that write a rule to user or workspace settings, such as ignoring that diagnostic type or downgrading an error to a warning.

If those actions are not available, edit `vizsla.diagnostics.slang.rules` manually. See [VS Code Settings](./vscode-settings.md#diagnostics) for the rule format.

## Project Files Are Not Scanned

Check the project manifest:

- Is the project manifest located at the workspace root? Prefer `vizsla.toml`; the legacy `vizsla_config.toml` still works but is deprecated, and `vizsla.toml` takes precedence when both exist.
- If `sources` is set, does the path pattern match the target files? `rtl/*.sv` only matches `.sv` files directly under `rtl`; recursive directories use `rtl/**`.
- Explicit `sources = []` disables workspace indexing.
- Does an `exclude` path pattern exclude the target file? Recursive directory exclusion uses `build/**`.
- Is the file extension `.v`, `.sv`, `.vh`, `.svh`, `.svi`, or `.map`?
- Did you open a subdirectory, changing the workspace root?

These path patterns use glob syntax with `*` and `**`: `*` does not cross directories, while `**` can. Use `/` as the separator in `sources` and `exclude`, even on Windows.

Trailing `/` depends on the field. `include_dirs = ["include"]` and `include_dirs = ["include/"]` both describe an include search directory, and the docs prefer the version without `/`. But `sources = ["rtl/"]` is not the recursive “all files under `rtl`” pattern; use `sources = ["rtl/**"]` for that.

The default `vizsla.toml` created by the extension writes `sources = []`. To index project files, add real `sources` patterns and add `include_dirs`, `defines`, `libraries`, or `top_modules` as needed. If a hand-written manifest omits `sources`, Vizsla scans the workspace as a best effort for basic reading and navigation, but it does not enable the full cross-file diagnostics view.

## Includes or Macros Do Not Work

Add include directories and macros to the project manifest:

```toml
defines = ["SYNTHESIS", "WIDTH=32"]
include_dirs = ["include", "rtl"]
```

If you set `include_dirs = []` explicitly, Vizsla does not fall back to `sources`.

## Formatting Produces No Result or Fails

The default formatter provider calls `verible-verilog-format`. If it is not installed locally, configure:

```json
{
  "vizsla.formatter.path": "D:\\tools\\verible\\verible-verilog-format.exe"
}
```

Formatter failures usually come from formatter stderr. Reduce custom `vizsla.formatter.args` first and verify with the default arguments.

## File Changes Do Not Trigger Refresh

The default `vizsla.files.watcher` is `client`, so Vizsla prefers VS Code watched-file notifications. If the client does not support dynamic watched files, Vizsla falls back to the server-side watcher.

If project file changes do not trigger a refresh:

```json
{
  "vizsla.files.watcher": "server"
}
```

`vizsla.files.excludeDirs` only accepts workspace-relative directories and does not support globs. Prefer the project manifest's `sources` / `exclude` shell globs for file selection. If you also want to reduce VS Code watcher events, configure VS Code's `files.watcherExclude` separately.

## Need More Detailed Server Logs

If the process starts but you need server-side logs, add `--log` and `--log_file` through `vizsla.server.additionalArgs`, then restart the language server. See [Server Self-Check Flow](./check-server.md) for the steps.
