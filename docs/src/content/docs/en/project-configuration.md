---
title: Project Configuration
description: Configure the vizsla.toml project manifest, source files, include directories, and macro definitions.
---

Vizsla prefers the project manifest file name `vizsla.toml`. The old file name `vizsla_config.toml` is still supported, but if both files exist, Vizsla reads `vizsla.toml`. The manifest must live under the workspace root or be passed explicitly as the project path. Field names must be current supported names; old or unknown fields are rejected.

## Complete Example

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/v1/vizsla.schema.json

top_modules = ["top"]

defines = [
  "SYNTHESIS",
  "DATA_WIDTH=32",
  "RESET_VALUE=0",
]

sources = [
  "rtl/**",
  "ip/local_ip/src/**/*.sv",
]

include_dirs = [
  "include",
  "rtl",
]

libraries = [
  "../common_cells",
  "../bus_ip",
]

exclude = [
  "build/**",
  "sim/work/**",
  "generated/tmp/**",
  "**/*_bb.v",
]
```

`sources` and `exclude` use workspace-relative shell glob semantics and always use `/` as the path separator. `include_dirs` and `libraries` are still resolved relative to the directory that contains the project manifest.

When a workspace contains Verilog/SystemVerilog files but no manifest, VS Code creates a default `vizsla.toml`:

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/v1/vizsla.schema.json
# Default startup manifest. Omitting sources enables best-effort indexing for navigation
# without semantic diagnostics. Fill shell globs, for example sources = ["rtl/**"]
# and include_dirs = ["include"], to enable semantic diagnostics.
# Set sources = [] to disable workspace indexing.
```

This default manifest lets Vizsla index Verilog/SystemVerilog files under the workspace in best-effort mode for read-only features such as navigation and references. It does not create a compile profile or run cross-file semantic diagnostics. Set `sources = []` explicitly to disable workspace indexing and return to the lower-cost mode that only handles opened files. For more accurate semantic diagnostics, add `sources` shell globs or `include_dirs` that match your project structure, then add `defines`, `libraries`, or `top_modules` as needed.

Vizsla only reads the manifest to build the project model and refreshes project information after the manifest changes. The schema directive at the top is a TOML comment and does not affect how Vizsla reads the manifest. The schema URL includes a version segment. If the manifest format changes later, newer Vizsla versions can generate a directive pointing to a newer schema while existing projects keep their current schema.

If you want to confirm the relationship between omitted `sources`, explicit `sources = []`, parsing, indexing, and diagnostics, read [Parsing and Analysis Model](./parsing-and-analysis.md).

## Fields

| Field | Type | Purpose |
| --- | --- | --- |
| `top_modules` | Array of strings | Declares the top-level modules for the current project. Vizsla writes them into the compile profile. |
| `defines` | Array of strings | Predefined macros. Supports both `"NAME"` and `"NAME=value"`. |
| `sources` | Array of shell globs, or omitted | Source file selection patterns. When omitted, Vizsla indexes the workspace root by default but does not enable semantic diagnostics. When set to `[]`, workspace indexing is disabled. |
| `include_dirs` | Array of paths, or omitted | Preprocessor include search directories. When omitted, defaults to scan roots inferred from `sources`. When set to `[]`, no fallback is used. |
| `libraries` | Array of paths | External libraries or dependency projects. Vizsla loads them as library workspaces. |
| `exclude` | Array of shell globs | Excludes matching source or header files from loaded files. |

## sources and include_dirs

`sources` decides which Verilog/SystemVerilog files Vizsla scans. It uses shell glob semantics and no longer treats the same string as both a path and a pattern. Common examples:

```toml
sources = ["rtl/**", "ip/**/*.sv", "tb/**/*.sv"]
```

`*`, `?`, `[]`, `{}`, and `**` are handled as shell globs. `*` does not cross `/`; use `**` for recursive directories. Patterns must be relative to the workspace root and must not use absolute paths, `..`, or backslashes. The current project loader reads these extensions:

- `.v`
- `.sv`
- `.vh`
- `.svh`
- `.svi`
- `.map`

`include_dirs` is passed to preprocessing and controls include search. If `include_dirs` is not configured explicitly, Vizsla uses scan roots inferred from `sources` patterns as include directories. For example, `sources = ["rtl/**/*.sv"]` uses `rtl` as the default include directory. If you set `include_dirs = []`, no fallback is used.

## defines

Each entry in `defines` is one macro definition:

```toml
defines = [
  "SYNTHESIS",
  "WIDTH=32",
  "MODE=fast sim",
]
```

Macro names must be valid identifiers. Macros with values are parsed as `NAME=value`; `value` may contain spaces.

## libraries

`libraries` declares dependency libraries. Each library path is parsed as another project path:

- If the library directory contains `vizsla.toml`, Vizsla loads the library with that manifest; otherwise it falls back to `vizsla_config.toml`.
- If the library directory has no manifest, Vizsla loads it as an unconfigured library.
- Dependencies participate in the current project's compile profile and transitive dependencies are supported.

## exclude

`exclude` uses the same workspace-relative shell glob semantics. It is useful for generated directories, simulation output, cache directories, or black-box stubs:

```toml
exclude = ["build/**", "out/**", "sim/work/**", "**/*_bb.v"]
```

`exclude = ["build"]` only matches the path named `build` itself and does not recursively exclude the directory contents. Use `build/**` for recursive directory exclusion. If you also want VS Code to reduce file watching, configure VS Code's `files.watcherExclude` as well. Vizsla's `exclude` and VS Code's watcher settings are separate mechanisms.

## Notes

- The recommended manifest file name is `vizsla.toml`; `vizsla_config.toml` is only a fallback for compatibility with old projects.
- Old fields such as `top_module`, `include`, or `macros` are not accepted.
- Vizsla does not automatically discover manifests from subdirectories. When you open `repo`, it only checks `repo/vizsla.toml` and then `repo/vizsla_config.toml`.
- Projects configured with `sources = []` can still load include directories, which is useful for header-only workspaces.
