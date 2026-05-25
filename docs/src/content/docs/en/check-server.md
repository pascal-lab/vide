---
title: Server Self-Check Flow
description: Step-by-step checks for the Vizsla bundled server or a custom server launch.
---

Use this page to confirm that the language server launch is healthy. Command and log entry points are in the [operations reference](./commands-status-logs.md); for a concrete failure, jump to [Troubleshooting](./troubleshooting.md).

## 1. Check the Status Bar

Start with the `Vizsla` status item on the right side of the VS Code status bar:

| State | Next step |
| --- | --- |
| Plain `Vizsla` text | The server has started. Hover to check whether project configuration loaded. |
| Spinner that does not finish | Check the `Vizsla Language Server` output channel. |
| Warning icon | Click the status item to confirm whether the workspace is missing a project manifest. |
| Error icon | Click the status item for the menu-top error, then open the output channel. |

Click the status item or run `Vizsla: Show Status` to open the status menu.

## 2. Open Language Server Output

Run `Vizsla: Show Language Server Output`, or choose `Show Output` from the status menu. A normal startup usually includes:

```text
[INFO] Vizsla extension activating...
[INFO] Platform: win32-x64
[INFO] Looking for bundled server at: ...
[INFO] Server command: ...
[INFO] Server args: ...
[INFO] Working directory: ...
[INFO] Language server started successfully
```

These lines confirm the platform, final server command, arguments, and working directory seen by the extension.

## 3. Verify the Server Version

Run `Vizsla: Show Server Version`. The extension uses the current server command, cwd, and environment, combines `vizsla.server.args` with `--version`, and does not append `vizsla.server.additionalArgs`. If `vizsla.server.command` is set, the custom command is used.

You can also test the binary directly in a terminal:

```powershell
vizsla --version
```

Windows custom-server example:

```powershell
D:\tools\vizsla\vizsla.exe --version
```

## 4. Check Bundled or Custom Server Selection

The default configuration uses the server bundled with the extension. The extension looks under the extension installation's `server` subdirectory:

- Windows: `vizsla.exe`
- macOS/Linux: `vizsla`

If `vizsla.server.command` is configured, the output channel should include:

```text
[INFO] Using custom server command: ...
```

Prefer an absolute path for custom servers, and validate it with `--version` first.

## 5. Enable a Server Log File When Needed

If the process starts but you need server-side logs, pass logging arguments through `vizsla.server.additionalArgs`:

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

After saving, choose `Restart` in the extension prompt or run `Vizsla: Restart Language Server`. If the process fails before it can read arguments, start with the `Vizsla Language Server` output channel.
