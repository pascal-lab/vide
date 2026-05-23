---
title: Commands, Status, and Logs
description: Vizsla command palette commands, status bar messages, and output channel.
---

## Command Palette Commands

The VS Code extension contributes these commands:

| Command | Purpose |
| --- | --- |
| `Vizsla: Show Language Server Output` | Opens the `Vizsla Language Server` output channel. |
| `Vizsla: Restart Language Server` | Stops and restarts the language server. |
| `Vizsla: Show Server Version` | Runs the server with `--version` and shows the first output line. |
| `Vizsla: Profile Diagnostics` | Runs one isolated diagnostics profiling pass for the workspace or current Verilog/SystemVerilog file and writes trace, summary, and flamegraph artifacts. |

## Status Bar

The extension shows the server state on the left side of the status bar:

| State | Meaning |
| --- | --- |
| `Vizsla Starting` | Creating and starting the language server. |
| `Vizsla Ready` | The language server has started. |
| `Vizsla Stopping` | Stopping the language server. |
| `Vizsla Stopped` | The language server has stopped. |
| `Vizsla Error` | Server startup failed. |

Click the status bar item to open the output channel. If you see `Vizsla Error`, start there.

## Output Channel

The extension output channel is named `Vizsla Language Server`. It records:

- Extension activation information.
- Extension installation path.
- Current platform and architecture.
- VS Code version.
- Server command, arguments, and working directory.
- Bundled server lookup result.
- Start, stop, restart, and version query results.

When `Vizsla: Profile Diagnostics` runs, the extension also opens the `Vizsla Profiling` output channel. It records the target, artifact directory, diagnostic request time, and generated file paths.

## Profile Diagnostics

Run `Vizsla: Profile Diagnostics`. The extension starts a temporary language server process, then sends one diagnostics request for the selected target:

- Workspace targets send `workspace/diagnostic` to measure the project-level diagnostics path.
- Current-file targets send `textDocument/diagnostic` to narrow the run to one file.

After the request finishes, the extension shuts the temporary process down. It does not restart or affect the language server used by your editor session.

The command generates:

| File | Description |
| --- | --- |
| `trace.json` | Chrome/Perfetto/Speedscope-compatible trace, and the input file for the interactive Speedscope viewer. |
| `summary.json` | Request timing, diagnostics summary, and top span summary. |
| `trace.folded` | Folded stack generated from the trace. |
| `flamegraph.svg` | Static flamegraph fallback. Interactive viewing opens `trace.json` in a VS Code tab backed by the bundled local Speedscope viewer. |
| `server.log` | Temporary language server log. |

## Query Server Version

Run `Vizsla: Show Server Version` from the command palette. The extension resolves the current server startup configuration and then runs:

```powershell
vizsla --version
```

If `vizsla.server.command` is configured, the version query uses that custom command. The extension places `vizsla.server.args` before `--version`.

## Restart After Configuration Changes

After these startup-related settings change, the extension prompts you to restart the language server:

- `vizsla.server.command`
- `vizsla.server.args`
- `vizsla.server.additionalArgs`
- `vizsla.server.cwd`
- `vizsla.trace.server`

Choose `Restart` in the prompt or run `Vizsla: Restart Language Server` manually.
