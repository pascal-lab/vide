---
title: Troubleshooting
description: Troubleshoot Vizsla status bar errors, server startup failures, diagnostics, and file watching.
---

## Status Bar Shows an Error or Warning Icon

First click the `Vizsla` status bar item to open the status menu. Project configuration errors appear at the top of that menu. You can also choose `Show Output` from the menu or run `Vizsla: Show Language Server Output` directly. Focus on:

- `Bundled Vizsla Language Server binary not found`
- `Unsupported platform-architecture combination`
- `Failed to start language server`
- `Server command`
- `Server args`
- `Working directory`

If the bundled server is missing, install the VSIX for the right platform or configure `vizsla.server.command` to point to a local server.

## Bundled Server Not Found

The extension looks for `server/vizsla.exe` or `server/vizsla` under its own installation directory. During local development, if you only ran `npm run compile`, you usually do not have a bundled server yet. You can package the extension:

```powershell
cd editors\vscode
npm run package:debug
```

Or configure a local server directly:

```json
{
  "vizsla.server.command": "D:\\Proj\\vizsla\\target\\release\\vizsla.exe"
}
```

## Custom command/args/cwd Startup Fails

Check these points:

- `vizsla.server.command` should use an absolute path.
- `vizsla.server.args` must be an array of strings.
- `vizsla.server.additionalArgs` must be an array of strings.
- If `vizsla.server.cwd` is set, it must point to an existing directory.
- Restart the language server after changing startup arguments.

Example:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.args": [],
  "vizsla.server.cwd": "D:\\work\\chip",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

## Diagnostics Are Too Frequent or Stale

The default `vizsla.diagnostics.update` is `onSave`, so diagnostics refresh when you save. This default is recommended for large projects.

If you want diagnostics while editing:

```json
{
  "vizsla.diagnostics.update": "onType"
}
```

If diagnostics do not update, save the file first. Then run `Vizsla: Restart Language Server` and check the output channel for project loading errors.

## Project Files Are Not Scanned

Check the project manifest:

- Is `vizsla.toml` located at the workspace root? The legacy `vizsla_config.toml` still works, but `vizsla.toml` takes precedence when both exist.
- If `sources` is set, does the shell glob match the target files? For recursive directories, use `rtl/**`; explicit `sources = []` disables workspace indexing.
- Does an `exclude` shell glob exclude the target file? Recursive directory exclusion uses `build/**`.
- Is the file extension `.v`, `.sv`, `.vh`, `.svh`, `.svi`, or `.map`?
- Did you open a subdirectory, changing the workspace root?

The VS Code extension only creates a default `vizsla.toml` when the workspace contains Verilog/SystemVerilog files and has no manifest:

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/v1/vizsla.schema.json
sources = []

# include_dirs = ["include"]
# defines = ["SYNTHESIS"]
# top_modules = ["top"]
# libraries = ["../common_cells"]
# exclude = ["build/**"]
```

This default manifest explicitly sets `sources = []`, so it does not scan the workspace root. To index project files or enable more accurate semantic diagnostics, add real `sources` shell globs or `include_dirs`, plus `defines`, `libraries`, or `top_modules` as needed. If a hand-written manifest omits `sources`, Vizsla enters best-effort workspace indexing mode. Vizsla does not automatically search parent or child directories for manifests.

## include or Macros Do Not Work

Add include directories and macros to the manifest:

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

Formatter failures usually come from formatter stderr. You can also reduce custom `vizsla.formatter.args` and verify with the default arguments first.

## File Watching Issues

The default `vizsla.files.watcher` is `client`, so Vizsla prefers VS Code watched-file notifications. If the client does not support dynamic watched files, Vizsla falls back to the server-side watcher.

If project file changes do not trigger a refresh:

```json
{
  "vizsla.files.watcher": "server"
}
```

`vizsla.files.excludeDirs` only accepts workspace-relative directories and does not support globs. Prefer the manifest's `sources` / `exclude` shell globs for file selection. If you also want to reduce VS Code watcher events, configure VS Code's `files.watcherExclude` separately.

## Debug with Logs

Write server logs to a file:

```json
{
  "vizsla.server.additionalArgs": [
    "--log",
    "debug",
    "--log_file",
    "D:\\work\\chip\\.vizsla\\server.log"
  ]
}
```

Then run `Vizsla: Restart Language Server`. If the server fails before reading arguments, still start with the VS Code `Vizsla Language Server` output channel.
