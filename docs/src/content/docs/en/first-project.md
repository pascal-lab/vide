---
title: First Project
description: Prepare your first Verilog/SystemVerilog project directory for Vizsla.
---

## Recommended Layout

Small projects can start with a simple structure:

```text
my-rtl/
  rtl/
    top.sv
    alu.sv
  include/
    defs.svh
```

Open `my-rtl` directly in VS Code:

```powershell
code D:\work\my-rtl
```

## What Happens Without a Manifest

If the opened workspace root contains Verilog/SystemVerilog files and has no `vizsla.toml` or legacy `vizsla_config.toml`, the extension creates a default `vizsla.toml` and shows a prompt:

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/v1/vizsla.schema.json
sources = []

# include_dirs = ["include"]
# defines = ["SYNTHESIS"]
# top_modules = ["top"]
# libraries = ["../common_cells"]
# exclude = ["build/**"]
```

This default manifest explicitly sets `sources = []`, so it does not scan source files under the workspace root, create a compile profile, or run cross-file semantic diagnostics. Later, you can add `sources` shell globs or `include_dirs`, plus `defines`, `libraries`, or `top_modules` as needed, to enable cross-file indexing and more accurate semantic diagnostics.

If the server is started by another client or from the command line and there is no `vizsla.toml` or `vizsla_config.toml`, or if a hand-written manifest omits `sources`, Vizsla enters best-effort indexing mode. Setting `sources = []` explicitly disables workspace indexing and keeps only syntax/parse diagnostics for opened files.

## When to Create a Manifest

As a project grows, we recommend editing `vizsla.toml` in the workspace root when:

- You only want to scan `rtl` and `include`, not simulation output, generated directories, or third-party caches.
- You need to set `defines`.
- You need to keep include directories separate from source directories.
- You have external library directories that should participate in analysis as dependencies.
- You want to declare `top_modules` explicitly.

Example:

```toml
top_modules = ["top"]
defines = ["SYNTHESIS", "DATA_WIDTH=32"]
sources = ["rtl/**"]
include_dirs = ["include"]
exclude = ["build/**", "out/**"]
```

The manifest is only read from the workspace root you opened. Vizsla does not automatically search parent or child directories for other manifests. If both `vizsla.toml` and `vizsla_config.toml` exist, `vizsla.toml` takes precedence.
