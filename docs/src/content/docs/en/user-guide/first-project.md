---
title: First Project
description: Prepare your first Verilog/SystemVerilog project directory for Vide.
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

Open `my-rtl` directly in VS Code. This directory is the workspace root: the top-level folder currently opened in VS Code.

```powershell
code D:\work\my-rtl
```

Then open a Verilog `.v`/`.vh` file or a SystemVerilog `.sv`/`.svh`/`.svi` file. After the extension is installed, VS Code should recognize the language, show syntax highlighting, and start Vide in the background.

## What Happens Without a Manifest

If the workspace root contains Verilog/SystemVerilog files and has no `vide.toml`, the extension prompts you to create a default `vide.toml`. After you choose to create it, the extension writes this file and reloads Vide:

```toml
#:schema https://vide.pascal-lab.net/schemas/v1/vide.schema.json
sources = []

# include_dirs = ["include"]
# defines = ["SYNTHESIS"]
# top_modules = ["top"]
# libraries = ["../common_cells"]
# exclude = ["build/**"]
```

This default manifest sets `sources = []`, which means: do not scan the whole workspace automatically yet. This is a safe way to open a project first, confirm that the extension starts, and then decide which directories should be analyzed.

If you write `vide.toml` by hand and omit `sources`, Vide best-effort indexes Verilog/SystemVerilog files under the workspace. Best-effort indexing can make read features such as navigation, references, and hover work where possible, but it is still not a full project configuration. Setting `sources = []` explicitly disables automatic workspace scanning.

Header files (`.vh`, `.svh`, `.svi`) usually participate in diagnostics after they are included by a `.v` or `.sv` source file. Opening a header directly, or only listing its directory in `include_dirs`, does not necessarily produce standalone header diagnostics.

## When to Edit the Project Manifest

Most users can start with the flow above. Edit `vide.toml` in the workspace root when:

- You want cross-file navigation, references, completion, and diagnostics to match the real project more closely.
- You only want to scan `rtl` and `include`, not simulation output, generated directories, or third-party caches.
- You need to set `defines`.
- You need to tell Vide where include files live.
- You have external library directories that should participate in analysis as dependencies.
- You want to declare `top_modules` explicitly.

A typical small project can use:

```toml
top_modules = ["top"]
defines = ["SYNTHESIS", "DATA_WIDTH=32"]
sources = ["rtl/**"]
include_dirs = ["include"]
exclude = ["build/**", "out/**"]
```

The manifest is only read from the workspace root you opened. Vide does not automatically search parent or child directories for other manifests.

Next, read [Project Configuration](../project-configuration/) to describe `sources`, `include_dirs`, `defines`, and exclusion rules for your project layout.
