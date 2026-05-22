---
title: Commands, Status, and Logs
description: Vizsla command palette commands, status bar messages, and output channel.
---

## Command Palette Commands

The VS Code extension contributes three commands:

| Command | Purpose |
| --- | --- |
| `Vizsla: Show Language Server Output` | Opens the `Vizsla Language Server` output channel. |
| `Vizsla: Restart Language Server` | Stops and restarts the language server. |
| `Vizsla: Show Server Version` | Runs the server with `--version` and shows the first output line. |

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
