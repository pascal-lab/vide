# Vide

Language support for Verilog and SystemVerilog powered by the Vide language server.

This extension bundles the `vide` language server for the target platform and contributes syntax highlighting, language configuration, diagnostics, navigation, completion, semantic tokens, formatting, and code actions for Verilog and SystemVerilog projects.

## Language Server Status

The status bar shows the Vide language server state: starting, ready, stopped, or error. Click the status item to open the Vide output channel.

The command palette also provides:

- `Vide: Show Language Server Output`
- `Vide: Restart Language Server`
- `Vide: Show Server Version`
- `Vide: Profile Diagnostics`

When server launch settings change, the extension prompts you to restart the language server so the new command, arguments, working directory, or trace setting can take effect.

`Vide: Profile Diagnostics` starts an isolated temporary language server session, runs either a workspace `workspace/diagnostic` request or a current-file `textDocument/diagnostic` request, writes trace, summary, and flamegraph artifacts, and opens the trace in a VS Code tab backed by the bundled Speedscope viewer when requested.

## Configuration

The extension launches the bundled server by default. Advanced users can override the server command with `vide.server.command` and pass additional arguments with `vide.server.args` or `vide.server.additionalArgs`.

Most language server behavior can be configured from VS Code Settings under `Vide`, including diagnostics, file watching, formatting, inlay hints, code lenses, semantic tokens, and signature help.
