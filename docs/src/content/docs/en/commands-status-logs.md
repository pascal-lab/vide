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
| `Vizsla: Reload Project Configuration` | Rereads project manifests and refreshes project information without restarting the language server. |
| `Vizsla: Show Status` | Opens the Vizsla status menu. |
| `Vizsla: Run Qihe Analysis` | Runs Qihe analysis for the current Verilog/SystemVerilog file. |
| `Vizsla: Profile Diagnostics` | Runs one isolated diagnostics profiling pass for the workspace or current Verilog/SystemVerilog file and writes trace, summary, and flamegraph artifacts. |

## Status Bar

The extension shows a status item named `Vizsla` on the right side of the VS Code status bar. The text is usually `Vizsla`; starting, stopping, or loading project configuration adds a spinner, a missing project manifest adds a warning icon, and server startup or project configuration failures add an error icon. Hover over the status item to see the current detail, such as whether the server is running, whether project configuration is loaded, whether no manifest exists, or whether project configuration failed.

Click the status item or run `Vizsla: Show Status` to open the status menu. The menu shows project configuration errors at the top and provides these actions:

- Open an existing `vizsla.toml`.
- Create `vizsla.toml` for workspace folders that do not have one.
- Run diagnostics profiling.
- Reload project configuration.
- Restart the language server.
- Open the `Vizsla Language Server` output channel.

When Qihe analysis runs, a separate `Qihe` status item appears to show running, finished, or failed state. If Qihe fails, clicking that item opens the `Vizsla Qihe` output channel.

## Output Channel

`Vizsla Language Server` records:

- Extension activation information.
- Extension installation path.
- Current platform and architecture.
- VS Code version.
- Server command, arguments, and working directory.
- Bundled server lookup result.
- Start, stop, restart, and version query results.

`Vizsla Qihe` records the target file, command progress, Qihe output, and failure information for `Vizsla: Run Qihe Analysis`. If Qihe fails, the `Show Qihe Output` action in the error notification opens this output channel.

When `Vizsla: Profile Diagnostics` runs, the extension also opens the `Vizsla Profiling` output channel. It records the target, artifact directory, diagnostic request time, and generated file paths.

## Run Qihe Analysis

Open a Verilog/SystemVerilog file and run `Vizsla: Run Qihe Analysis`. The extension sends the request to the current language server and invokes Qihe according to the `vizsla.qihe.*` settings. See [VS Code Settings](./vscode-settings.md#qihe) for the Qihe command, compile arguments, run arguments, and automatic manifest-derived argument behavior.

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
