---
title: Check the Server
description: Check whether the Vizsla bundled server or custom server starts correctly.
---

## Check the Status Bar

The most direct signal is the `Vizsla` status item on the right side of the VS Code status bar:

- Plain `Vizsla` text with no error icon: the server has started; hover to see project configuration status.
- A spinner that stays for a long time: server startup, shutdown, or project configuration loading is stuck, so check the output channel.
- An error icon: server startup or project configuration loading failed.
- A warning icon: usually means the current workspace has no project manifest.

Click the status item to open the `Vizsla Status` menu. From there, run `Show Output` to open the `Vizsla Language Server` output channel.

## Check the Output Channel

Run `Vizsla: Show Language Server Output` and look for entries like:

```text
[INFO] Vizsla extension activating...
[INFO] Platform: win32-x64
[INFO] Looking for bundled server at: ...
[INFO] Server command: ...
[INFO] Server args: ...
[INFO] Working directory: ...
[INFO] Language server started successfully
```

If you see that the bundled server was not found, the current VSIX does not contain a usable server or the platform does not match. Install the VSIX for the right platform or configure `vizsla.server.command`.

## Verify the Bundled Server

By default, the extension looks for the server under the `server` subdirectory of the extension installation:

- Windows: `vizsla.exe`
- macOS/Linux: `vizsla`

If it is found, the output channel records the bundled server path. On non-Windows platforms, the extension also checks executable permissions and tries to set them to `755`.

## Verify a Custom Server

After configuring a custom server, the output channel should include:

```text
[INFO] Using custom server command: ...
```

We recommend testing it directly in a terminal first:

```powershell
D:\tools\vizsla\vizsla.exe --version
```

Then use the same path in settings:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe"
}
```

## Use vizsla --version

The server binary supports `--version`:

```powershell
vizsla --version
```

The version format includes the Cargo package version and distinguishes `DEBUG` and `RELEASE` builds.

## Enable Server Logs

The server supports `--log` and `--log_file`:

```powershell
vizsla --log debug --log_file .\.vizsla\server.log
```

Pass these arguments through the VS Code extension:

```json
{
  "vizsla.server.additionalArgs": [
    "--log",
    "debug",
    "--log_file",
    "D:\\work\\my-rtl\\.vizsla\\server.log"
  ]
}
```

After changing startup arguments, run `Vizsla: Restart Language Server`.
