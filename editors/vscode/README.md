# Vizsla LSP

Language support for Verilog and SystemVerilog powered by the Vizsla language server.

This extension bundles the `vizsla` language server for the target platform and contributes syntax highlighting, language configuration, diagnostics, navigation, completion, semantic tokens, formatting, and code actions for Verilog and SystemVerilog projects.

## Configuration

The extension launches the bundled server by default. Advanced users can override the server command with `vizslaLsp.server.command` and pass additional arguments with `vizslaLsp.server.args` or `vizslaLsp.server.additionalArgs`.

Diagnostics can be configured through `vizsla.diagnostics`, including parse diagnostics, semantic diagnostics, and slang warning or severity rules.
