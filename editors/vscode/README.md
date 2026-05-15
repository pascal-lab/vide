# Vizsla LSP

Language support for Verilog and SystemVerilog powered by the Vizsla language server.

This extension bundles the `vizsla` language server for the target platform and contributes syntax highlighting, language configuration, diagnostics, navigation, completion, semantic tokens, formatting, and code actions for Verilog and SystemVerilog projects.

## Language Server Status

The status bar shows the Vizsla language server state: starting, ready, stopped, or error. Click the status item to open the Vizsla output channel.

The command palette also provides:

- `Vizsla: Show Language Server Output`
- `Vizsla: Restart Language Server`
- `Vizsla: Show Server Version`

When server launch settings change, the extension prompts you to restart the language server so the new command, arguments, working directory, or trace setting can take effect.

## Configuration

The extension launches the bundled server by default. Advanced users can override the server command with `vizsla.server.command` and pass additional arguments with `vizsla.server.args` or `vizsla.server.additionalArgs`.

Diagnostics can be configured through `vizsla.diagnostics`, including parse diagnostics, semantic diagnostics, and slang warning or severity rules.
