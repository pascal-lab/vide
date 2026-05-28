---
title: Server Self-Check Flow
description: Step-by-step checks for the Vide bundled server or a custom server launch.
---

Use this page only for the question "did the extension start the language server?" Command IDs, status item meanings, and output channel inventory live in [Commands, Status, and Logs](../../user-guide/commands-status-logs/). If the process starts but behavior is still wrong, continue with [Advanced Troubleshooting](../troubleshooting/).

## 1. Open the Status Menu

Click `Vide` on the right side of the VS Code status bar, or run `Vide: Show Status`. Use it only to choose the next entry point:

| What you see | Next step |
| --- | --- |
| No menu-top error, and hover text says the server is connected | Startup is usually healthy; if diagnostics or navigation are missing, check project configuration next. |
| The menu shows a language server error | Note the error text, then open the `Vide Language Server` output channel. |
| The status keeps showing startup progress | Open the `Vide Language Server` output channel directly. |
| The menu says no project configuration file is available | Startup is usually not the issue; create or open the project configuration file first. |

## 2. Check Language Server Output

Run `Vide: Show Language Server Output`, or choose `Show Output` from the status menu. A normal startup usually includes:

```text
[INFO] Vide extension activating...
[INFO] Platform: win32-x64
[INFO] Looking for bundled server at: ...
[INFO] Server command: ...
[INFO] Server args: ...
[INFO] Working directory: ...
[INFO] Language server started successfully
```

These lines confirm the platform, final server command, arguments, and working directory seen by the extension.

If the output does not include `Language server started successfully`, start from the last error. Common branches are:

- Bundled server not found: check the extension package or target platform.
- Custom command missing or not executable: validate the custom server path.
- Process starts and exits immediately: run the version command first, then decide whether server-side logs are needed.

## 3. Verify the Server Command

Run `Vide: Show Server Version`. If it fails too, the server command, working directory, or base arguments currently used by the extension are not runnable yet.

You can also validate the same binary directly in a terminal:

```powershell
vide --version
```

Windows custom-server example:

```powershell
D:\tools\vide\vide.exe --version
```

## 4. Identify Bundled vs. Custom Server Selection

The default configuration uses the server bundled with the extension. The extension looks under the extension installation's `server` subdirectory:

- Windows: `vide.exe`
- macOS/Linux: `vide`

If `vide.server.command` is configured, the output channel should include:

```text
[INFO] Using custom server command: ...
```

Prefer an absolute path for custom servers, and validate it with `--version` first. If the custom command works in a terminal but fails from the extension, compare `vide.server.cwd`, `vide.server.args`, and PATH differences.

## 5. Process Starts but Behavior Is Still Wrong

If the output channel says the server started successfully, the launch path has usually passed. Continue with [Advanced Troubleshooting](../troubleshooting/) for detailed server logs, or return to the feature page for project configuration, diagnostics, navigation, or Qihe settings.
