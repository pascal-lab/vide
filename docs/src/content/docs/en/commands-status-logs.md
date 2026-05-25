---
title: "Operations Reference: Commands, Status, and Logs"
description: Quick reference for Vizsla command IDs, status bar items, and output channels.
---

Use this page as an operations reference. To check whether the server starts correctly, follow [Server Self-Check Flow](./check-server.md). For failures, start from the symptom in [Troubleshooting](./troubleshooting.md).

## Command Palette Commands

The VS Code extension contributes these commands. Use the Command Palette title in daily work; command IDs are mainly for keybindings or scripts.

| Command ID | Command Palette title | Purpose |
| --- | --- | --- |
| `vizsla.showOutput` | `Vizsla: Show Language Server Output` | Opens the `Vizsla Language Server` output channel. |
| `vizsla.restartServer` | `Vizsla: Restart Language Server` | Stops and restarts the current language server. |
| `vizsla.showServerVersion` | `Vizsla: Show Server Version` | Runs the current server command with the current cwd and environment, combines `vizsla.server.args` with `--version`, and does not append `vizsla.server.additionalArgs`. |
| `vizsla.reloadWorkspace` | `Vizsla: Reload Project Configuration` | Rereads project manifests and refreshes project information without restarting the server. |
| `vizsla.showStatus` | `Vizsla: Show Status` | Opens the Vizsla status menu. |
| `vizsla.runQiheAnalysis` | `Vizsla: Run Qihe Analysis` | Runs Qihe analysis for the current local Verilog/SystemVerilog file. |
| `vizsla.profileDiagnostics` | `Vizsla: Profile Diagnostics` | Starts a temporary language server and runs one diagnostics profiling pass for the workspace or current file. |

`vizsla.runQiheAnalysis` is available only for local files whose extension is `.v`, `.vh`, `.sv`, `.svh`, or `.svi`.

## Status Bar

The main status item is named `Vizsla` and appears on the right side of the VS Code status bar. It reports language server and project configuration state:

| State | Meaning |
| --- | --- |
| Plain `Vizsla` text | The server has started; hover to see project configuration state. |
| Spinner | Server startup, shutdown, or project configuration loading is in progress. |
| Warning icon | Usually means the current workspace has no project manifest. |
| Error icon | Server startup or project configuration loading failed. |

Click the `Vizsla` status item or run `Vizsla: Show Status` to open the status menu. Depending on the current project state, the menu can show `Open Manifest`, `Create Manifest`, `Profile Diagnostics`, `Reload Project`, `Restart Language Server`, and `Show Output`.

When Qihe analysis runs, a separate `Qihe` status item appears. If Qihe fails, clicking that item opens the `Vizsla Qihe` output channel.

## Output Channels

| Output channel | Records |
| --- | --- |
| `Vizsla Language Server` | Extension activation, platform, VS Code version, server command, args, cwd, bundled server lookup result, start/stop/restart, and version queries. |
| `Vizsla Qihe` | Target file, command progress, Qihe output, and failure details for `vizsla.runQiheAnalysis`. |
| `Vizsla Profiling` | Target, artifact directory, diagnostics request time, and generated file paths for `vizsla.profileDiagnostics`. |

## Advanced: Diagnostics Profiling Artifacts

`Vizsla: Profile Diagnostics` is mainly for large-project troubleshooting. It generates:

| File | Description |
| --- | --- |
| `trace.json` | Chrome/Perfetto/Speedscope-compatible trace; the bundled Speedscope viewer opens this file. |
| `summary.json` | Request timing, diagnostics summary, and top span summary. |
| `trace.folded` | Folded stack generated from the trace. |
| `flamegraph.svg` | Static flamegraph. |
| `server.log` | Temporary language server log. |

Profiling uses an isolated temporary language server process, so it does not restart or affect the language server used by your editor session.

## Server Launch Setting Changes

After these settings change, the extension prompts you to `Restart`:

- `vizsla.server.command`
- `vizsla.server.args`
- `vizsla.server.additionalArgs`
- `vizsla.server.cwd`
- `vizsla.trace.server`

Choose `Restart` in the prompt or run `vizsla.restartServer` manually.
