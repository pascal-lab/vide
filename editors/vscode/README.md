# Vizsla

Language support for Verilog and SystemVerilog powered by the Vizsla language server.

This extension bundles the `vizsla` language server for the target platform and contributes syntax highlighting, language configuration, diagnostics, navigation, completion, semantic tokens, formatting, and code actions for Verilog and SystemVerilog projects.

## Language Server Status

The status bar shows the Vizsla language server state: starting, ready, stopped, or error. Click the status item to open the Vizsla output channel.

The command palette also provides:

- `Vizsla: Show Language Server Output`
- `Vizsla: Restart Language Server`
- `Vizsla: Show Server Version`
- `Vizsla: Profile Diagnostics`

When server launch settings change, the extension prompts you to restart the language server so the new command, arguments, working directory, or trace setting can take effect.

`Vizsla: Profile Diagnostics` starts an isolated temporary language server session, runs either a workspace `workspace/diagnostic` request or a current-file `textDocument/diagnostic` request, writes trace, summary, and interactive flamegraph artifacts, and opens the `Vizsla Profiling` output channel with the generated paths.

## Configuration

The extension launches the bundled server by default. Advanced users can override the server command with `vizsla.server.command` and pass additional arguments with `vizsla.server.args` or `vizsla.server.additionalArgs`.

Most language server behavior can be configured from VS Code Settings under `Vizsla`, including diagnostics, file watching, formatting, inlay hints, code lenses, semantic tokens, and signature help.
