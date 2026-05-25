---
title: VS Code Settings
description: Configuration reference for the Vizsla VS Code extension.
---

All settings are under the `vizsla.*` namespace. You can search for `Vizsla` in the VS Code Settings UI or edit `settings.json` directly.

## Server

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.server.command` | `null` | Custom language server command. When empty, the bundled server is used. |
| `vizsla.server.args` | `[]` | Arguments passed before the server command's additional arguments. |
| `vizsla.server.additionalArgs` | `[]` | Arguments appended when starting the server. |
| `vizsla.server.cwd` | `null` | Server working directory. Defaults to the first workspace folder, or the extension directory when there is no workspace. |
| `vizsla.trace.server` | `"off"` | LSP communication trace. Options: `"off"`, `"messages"`, `"verbose"`. |

Example:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

## Qihe

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.qihe.command` | `"qihe"` | Command used to invoke Qihe. It must be available on the environment `PATH` seen by VS Code, or it can be an absolute path. |
| `vizsla.qihe.autoConfigureArgsFromManifest` | `true` | Automatically adds the Qihe compile mode and forwarded slang options from `vizsla.toml`. |
| `vizsla.qihe.compileArgs` | `[]` | Arguments inserted after `qihe compile`, useful for manually selecting the compile mode or forwarding slang options. |
| `vizsla.qihe.runArgs` | `["-g", "std"]` | Arguments appended when `Vizsla: Run Qihe Analysis` runs `qihe run`. |

By default, the extension derives the Qihe compile mode, top module, include directories, and macro definitions from the current `vizsla.toml`. Projects that already manage Qihe arguments through scripts or custom settings can disable `vizsla.qihe.autoConfigureArgsFromManifest` and rely only on `vizsla.qihe.compileArgs` and `vizsla.qihe.runArgs`.

Example:

```json
{
  "vizsla.qihe.command": "D:\\tools\\qihe\\qihe.exe",
  "vizsla.qihe.autoConfigureArgsFromManifest": false,
  "vizsla.qihe.compileArgs": ["--mode", "sv", "--", "-I", "include"],
  "vizsla.qihe.runArgs": ["-g", "std"]
}
```

## Files

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.files.excludeDirs` | `[]` | Workspace-relative directory exclusion list. Globs are not supported here; file-selection globs belong in the manifest's `sources` / `exclude`. |
| `vizsla.files.watcher` | `"client"` | File watching mode. Options: `"client"`, `"notify"`, `"server"`. |

`client` prefers VS Code watched-file notifications. In the current server configuration, if the client does not support dynamic watched files, Vizsla falls back to the server-side watcher. Both `notify` and `server` use the server-side watching path.

## Workspace

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.workspace.auto.reload` | `true` | Automatically refresh project information after the project manifest changes. |

## Scope

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.scope.visibility` | `"private"` | Controls visibility of symbols inside scopes. Options: `"private"`, `"public"`. |

This setting affects references, rename, and document highlights.

## Formatter and Formatting

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.formatter.provider` | `"verible"` | Formatter backend. Currently supports `verible`, which calls external `verible-verilog-format`. |
| `vizsla.formatter.path` | `null` | Executable path used by the `verible` provider. When empty, Vizsla looks for `verible-verilog-format`. |
| `vizsla.formatter.args` | `["--failsafe_success=false"]` | Arguments passed to `verible-verilog-format`. |
| `vizsla.formatting.on.enter` | `true` | Enables formatting behavior when pressing Enter. |
| `vizsla.formatting.in.comments` | `true` | Enables Enter assistance inside comments. |
| `vizsla.formatting.indent.width` | `4` | Fallback indentation width when the editor does not provide formatting options. |

`Format Document`, `Format Selection`, and on-type formatting requests prefer the editor-provided `tabSize`. The `verible` provider appends `--indentation_spaces=<N>` for the current indentation width after formatter args.

## Inlay Hints

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.inlayHints.port.connection.enable` | `true` | Shows port connection inlay hints. |
| `vizsla.inlayHints.parameter.assignment.enable` | `true` | Shows parameter assignment inlay hints. |
| `vizsla.inlayHints.end.structure.enable` | `true` | Shows end-structure name hints. |

## Lens

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.lens.instantiations.enable` | `true` | Shows module instance code lens. |

## Semantic Tokens

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.semantic.tokens.port.clk.rst.enable` | `true` | Enables dedicated semantic token modifiers for clock/reset ports. |
| `vizsla.semantic.tokens.port.input.output.enable` | `true` | Enables dedicated semantic token modifiers for input/output ports. |

## Diagnostics

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.diagnostics.enable` | `true` | Enables all Vizsla diagnostics. |
| `vizsla.diagnostics.update` | `"onSave"` | Diagnostics refresh timing. Options: `"onSave"`, `"onType"`. |
| `vizsla.diagnostics.parse.enable` | `true` | Enables syntax and parse diagnostics. |
| `vizsla.diagnostics.semantic.enable` | `true` | Enables compile and semantic diagnostics. |
| `vizsla.diagnostics.slang.warnings` | `[]` | slang warning options, such as `default`, `everything`, `none`, `error`, `no-<name>`, `error=<name>`. |
| `vizsla.diagnostics.slang.rules` | `[]` | Diagnostic filter or severity override rules. |

`vizsla.diagnostics.slang.warnings` is passed to slang parse/semantic diagnostics. It follows slang `-W...` warning option semantics, but VS Code settings omit the leading `-W`: for example, `everything` maps to `-Weverything`, `no-unused` maps to `-Wno-unused`, and `error=width-trunc` maps to `-Werror=width-trunc`.

To look up warning names, warning groups, or warning flag semantics, prefer the slang documentation:

- [slang Warning Reference](https://sv-lang.com/warning-ref.html): complete warning names and groups.
- [slang Command Line Reference](https://sv-lang.com/command-line-ref.html): behavior of `-Wfoo`, `-Wno-foo`, `-Wnone`, `-Weverything`, `-Werror`, and related warning options.
- [slang User Manual](https://sv-lang.com/user-manual.html): source-level diagnostic control such as `pragma diagnostic` and `slang lint_off` / `lint_on`.

Selectors in `vizsla.diagnostics.slang.rules` support:

- `code:<subsystem>:<code>`
- `option:<name>`
- `group:<name>`
- `source:parse`
- `source:semantic`

Example:

```json
{
  "vizsla.diagnostics.slang.rules": [
    { "selector": "source:parse", "severity": "warning" },
    { "selector": "option:unconnected-port", "severity": "ignore" }
  ]
}
```

`severity` can be `ignore`, `info`, `warning`, `error`, or `fatal`.

## Signature Help

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.signature.help.params.only` | `false` | Shows only parameter-related signature help. |
