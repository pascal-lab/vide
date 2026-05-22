---
title: Quick Start
description: Install the Vizsla extension and confirm that the core IDE features work.
---

Follow these steps to start using Vizsla quickly.

## 1. Install the Extension

Search for the display name `Vizsla` in the VS Code Extensions view and install it.

## 2. Open a Project Directory

Open the directory that contains your RTL source code in VS Code. If there is no `vizsla.toml` or legacy `vizsla_config.toml`, the extension creates a default `vizsla.toml` and shows a prompt:

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/v1/vizsla.schema.json
# Default startup manifest. Omitting sources enables best-effort indexing for navigation
# without semantic diagnostics. Fill shell globs, for example sources = ["rtl/**"]
# and include_dirs = ["include"], to enable semantic diagnostics.
# Set sources = [] to disable workspace indexing.
```

This default manifest indexes Verilog/SystemVerilog files under the workspace so read-only features such as cross-file navigation and references work out of the box. It does not create a compile profile or run cross-file semantic diagnostics. Set `sources = []` explicitly to disable workspace indexing.

## 3. Check the Status Bar

After the extension activates, the left side of the status bar shows the Vizsla server state:

- `Vizsla Starting`: the server is starting.
- `Vizsla Ready`: the server has started.
- `Vizsla Error`: startup failed. Click the status bar item to open the output channel.
- `Vizsla Stopped`: the server has stopped.

## 4. Open a Verilog/SystemVerilog File

Open a `.v`, `.vh`, `.sv`, `.svh`, or `.svi` file. VS Code should recognize it as Verilog or SystemVerilog and enable syntax highlighting and language services.

## 5. Try the Core Features

You can verify features in this order:

1. Write an obvious syntax error and check diagnostics in the `Problems` panel.
2. Run `Go to Definition` or `Go to Declaration` on a module name, signal name, or instance name.
3. Trigger completion inside instance port connections, parameter assignments, expressions, or preprocessor positions.
4. Hover over a symbol to view its information.
5. Run `Format Document`. Formatting uses `verible-verilog-format` by default. If it is not installed locally, configure `vizsla.formatter.path` or skip this check for now.

If VS Code prompts you to restart the language server after a configuration change, choose `Restart`.

When you need cross-file semantic diagnostics and more accurate port or parameter assistance, add real `sources` or `include_dirs` to `vizsla.toml`, then add `defines`, `libraries`, or `top_modules` as needed.
