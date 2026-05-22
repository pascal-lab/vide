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
# Default startup manifest. Omitting sources enables best-effort indexing for navigation
# without semantic diagnostics. Fill shell globs, for example sources = ["rtl/**"]
# and include_dirs = ["include"], to enable semantic diagnostics.
# Set sources = [] to disable workspace indexing.
```

This default manifest indexes Verilog/SystemVerilog files under the workspace root in best-effort mode, so read-only features such as go to definition and references work out of the box. It does not create a compile profile and does not run cross-file semantic diagnostics. Later, you can add `sources` shell globs or `include_dirs`, plus `defines`, `libraries`, or `top_modules` as needed, to enable more accurate semantic diagnostics.

If the server is started by another client or from the command line and there is no `vizsla.toml` or `vizsla_config.toml`, Vizsla also enters best-effort indexing mode. Setting `sources = []` explicitly disables workspace indexing and keeps only syntax/parse diagnostics for opened files.

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
