---
title: VS Code Settings Reference
description: Compact configuration reference for the Vide VS Code extension.
---

All settings are under the `vizsla.*` namespace. Search for `Vide` in the VS Code Settings UI or edit `settings.json` directly.

Start from a daily task: [Language Support](../../user-guide/daily-use/language-support/) / [Diagnostics](../../user-guide/daily-use/diagnostics/) / [Navigation and Reading](../../user-guide/daily-use/navigation/) / [Editing Assistance](../../user-guide/daily-use/editing-assistance/) / [Formatting](../../user-guide/daily-use/formatting/) / [Structure Help](../../user-guide/daily-use/structure/) / [Qihe](../../user-guide/daily-use/qihe/).

## Common Settings Quick Reference

Most users only change these:

| Goal | Common settings |
| --- | --- |
| Point VS Code at a local Qihe executable | `vizsla.qihe.command` |
| Point Vide at `verible-verilog-format` | `vizsla.formatter.path` |
| Refresh diagnostics on save or while typing | `vizsla.diagnostics.update` |
| Toggle port, parameter, and end-structure inlay hints | `vizsla.inlayHints.*` |
| Toggle instance counts above module declarations | `vizsla.lens.instantiations.enable` |
| Refresh project information after manifest changes | `vizsla.workspace.auto.reload` |

Server launch, file watching, diagnostic rules, and protocol tracing are mostly for troubleshooting or development. Keep their defaults unless you have a specific reason to change them.

## Server

These settings replace or debug the background language server. A normal installation usually does not need them.

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.server.command` | `null` | Custom language server command. When empty, the bundled server is used. |
| `vizsla.server.args` | `[]` | Arguments passed before the server command's additional arguments. |
| `vizsla.server.additionalArgs` | `[]` | Arguments appended when starting the server, commonly used for `--log` / `--log_file`. |
| `vizsla.server.cwd` | `null` | Server working directory. Defaults to the first workspace folder, or the extension directory when there is no workspace. |
| `vizsla.trace.server` | `"off"` | LSP communication trace. Options: `"off"`, `"messages"`, `"verbose"`. |

After these server launch settings change, the extension prompts you to `Restart`: `vizsla.server.command`, `vizsla.server.args`, `vizsla.server.additionalArgs`, `vizsla.server.cwd`, `vizsla.trace.server`.

Example:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

## Qihe

If you do not use `Vide: Run Qihe Analysis`, keep these defaults.

Related daily-use page: [Qihe](../../user-guide/daily-use/qihe/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.qihe.command` | `"qihe"` | Command used to invoke Qihe. It must be available on the `PATH` seen by VS Code, or it can be an absolute path. |
| `vizsla.qihe.autoConfigureArgsFromManifest` | `true` | Automatically adds the Qihe compile mode and forwarded slang options from the current project manifest. |
| `vizsla.qihe.compileArgs` | `[]` | Arguments inserted after `qihe compile`, used for manual compile mode selection or forwarded slang options. |
| `vizsla.qihe.runArgs` | `["-g", "std"]` | Arguments appended when `Vide: Run Qihe Analysis` runs `qihe run`. |

`Vide: Run Qihe Analysis` is available only for local Verilog/SystemVerilog files. By default, Vide derives the Qihe compile mode, top module, include directories, and macro definitions from the current project manifest; the recommended file name is `vizsla.toml`, and the legacy `vizsla_config.toml` name is still supported but deprecated. If your project already manages those arguments through scripts, disable automatic derivation and configure `compileArgs` / `runArgs` explicitly.

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

These settings are mainly for file-watching troubleshooting. Use `sources` / `exclude` in `vizsla.toml` for project file selection.

Related daily-use page: [Language Support](../../user-guide/daily-use/language-support/); project file selection is covered in [Project Configuration](../../user-guide/project-configuration/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.files.excludeDirs` | `[]` | Workspace-relative directory exclusion list. Globs are not supported here; file-selection globs belong in the project manifest's `sources` / `exclude`. |
| `vizsla.files.watcher` | `"client"` | File watching mode. Options: `"client"`, `"notify"`, `"server"`. |

`client` prefers VS Code watched-file notifications. If the client does not support dynamic watched files, Vide falls back to the server-side watcher. Both `notify` and `server` use the server-side watching path.

## Workspace

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.workspace.auto.reload` | `true` | Automatically refresh project information after the project manifest changes. |

## Scope

This setting affects reading features such as navigation, references, and rename. Keep the default when unsure.

Related daily-use page: [Navigation and Reading](../../user-guide/daily-use/navigation/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.scope.visibility` | `"private"` | Controls visibility of symbols inside scopes. Options: `"private"`, `"public"`. |

This setting affects references, rename, and document highlights.

## Formatter and Formatting

Configure these only if you use Verilog/SystemVerilog formatting. Vide does not bundle `verible-verilog-format`.

Related daily-use page: [Formatting](../../user-guide/daily-use/formatting/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.formatter.provider` | `"verible"` | Formatter backend. Currently supports `verible`, which calls external `verible-verilog-format`. |
| `vizsla.formatter.path` | `null` | Executable path used by the `verible` provider. When empty, Vide looks for `verible-verilog-format`. |
| `vizsla.formatter.args` | `["--failsafe_success=false"]` | Arguments passed to `verible-verilog-format`. |
| `vizsla.formatting.on.enter` | `true` | Enables formatting behavior when pressing Enter. |
| `vizsla.formatting.in.comments` | `true` | Enables Enter assistance inside comments. |
| `vizsla.formatting.indent.width` | `4` | Fallback indentation width when the editor does not provide formatting options. |

The `verible` provider appends `--indentation_spaces=<N>` for the current indentation width after formatter args.

## Inlay Hints

Inlay hints appear directly in the editor and are useful for reading port and parameter connections.

Related daily-use page: [Structure Help](../../user-guide/daily-use/structure/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.inlayHints.port.connection.enable` | `true` | Shows port connection inlay hints. |
| `vizsla.inlayHints.parameter.assignment.enable` | `true` | Shows parameter assignment inlay hints. |
| `vizsla.inlayHints.end.structure.enable` | `true` | Shows end-structure name hints. |

## Lens

Instance-count lens entries appear above module declarations.

Related daily-use page: [Structure Help](../../user-guide/daily-use/structure/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.lens.instantiations.enable` | `true` | Shows module instance code lens. |

## Semantic Tokens

Semantic tokens can make port direction, clock/reset ports, and read/write positions easier to distinguish when your theme supports them.

Related daily-use pages: [Language Support](../../user-guide/daily-use/language-support/) and [Structure Help](../../user-guide/daily-use/structure/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.semantic.tokens.port.clk.rst.enable` | `true` | Enables dedicated semantic token modifiers for clock/reset ports. |
| `vizsla.semantic.tokens.port.input.output.enable` | `true` | Enables dedicated semantic token modifiers for input/output ports. |

## Diagnostics

Diagnostics appear in the VS Code `Problems` panel and as editor underlines.

Related daily-use page: [Diagnostics](../../user-guide/daily-use/diagnostics/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.diagnostics.enable` | `true` | Enables all Vide diagnostics. |
| `vizsla.diagnostics.update` | `"onSave"` | Diagnostics refresh timing. Options: `"onSave"`, `"onType"`. |
| `vizsla.diagnostics.parse.enable` | `true` | Enables syntax and parse diagnostics. |
| `vizsla.diagnostics.semantic.enable` | `true` | Enables compile and semantic diagnostics. |
| `vizsla.diagnostics.slang.warnings` | `[]` | slang warning options, such as `default`, `everything`, `none`, `error`, `no-<name>`, `error=<name>`. |
| `vizsla.diagnostics.slang.rules` | `[]` | Diagnostic filter or severity override rules. |

`vizsla.diagnostics.slang.warnings` follows slang `-W...` semantics, but VS Code settings omit the leading `-W`. `vizsla.diagnostics.slang.rules` selectors support `code:<subsystem>:<code>`, `option:<name>`, `group:<name>`, `source:parse`, and `source:semantic`; `severity` can be `ignore`, `info`, `warning`, `error`, or `fatal`.

Example:

```json
{
  "vizsla.diagnostics.slang.rules": [
    { "selector": "source:parse", "severity": "warning" },
    { "selector": "option:unconnected-port", "severity": "ignore" }
  ]
}
```

## Signature Help

Signature help is used for instance port connections and parameter assignment lists.

Related daily-use page: [Editing Assistance](../../user-guide/daily-use/editing-assistance/).

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.signature.help.params.only` | `false` | Shows only parameter-related signature help. |
