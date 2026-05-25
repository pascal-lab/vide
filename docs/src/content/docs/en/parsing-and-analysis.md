---
title: Parsing and Analysis Model
description: Why Vizsla can sometimes only read files and sometimes run full project analysis.
---

Read this page when you run into questions like:

- Why can navigation work without `vizsla.toml`, while cross-file diagnostics are incomplete?
- Why does `sources = []` stop Vizsla from scanning the workspace automatically?
- Why does a header listed through `include_dirs` not always get standalone diagnostics?
- Why does Qihe sometimes use project analysis and sometimes fall back to single-file analysis?

Three terms are useful:

- Best-effort indexing: when there is no full project configuration, Vizsla still tries to read Verilog/SystemVerilog files in the workspace so navigation, references, hover, and completion can work where possible.
- Project analysis: the project view Vizsla builds from `vizsla.toml`, including `sources`, `include_dirs`, `defines`, `libraries`, and `top_modules`. Cross-file diagnostics and Qihe project analysis depend on it.
- Diagnostics: errors, warnings, and hints in the VS Code `Problems` panel. Single-file syntax issues can be reported without full project configuration; cross-file semantic issues need project analysis.

## sources Is the Main Switch

`sources` decides whether files are treated as part of the project.

| Configuration | What Vizsla reads | Project analysis |
| --- | --- | --- |
| No project manifest | Best-effort indexes the workspace | Not created |
| Manifest exists but omits `sources` | Best-effort indexes the workspace | Not created for those default-indexed files |
| Omits `sources`, but sets `include_dirs` | Best-effort indexes the workspace and loads include directories | Include directories can be used by project analysis; default-indexed files do not participate |
| `sources = []` | Does not scan the workspace automatically | Not created |
| `sources = []` with `include_dirs` | Loads only include directories | Include directories can be used by project analysis |
| `sources = ["rtl/**"]` | Loads matching source files | Created |

A short way to remember it: omitted `sources` means "read the workspace for me, but do not pretend it is fully configured"; `sources = []` means "do not scan the workspace automatically"; `sources = ["rtl/**"]` means "these files belong to my project."

`include_dirs` only controls include search. If you set `sources` explicitly but omit `include_dirs`, Vizsla infers a default include directory from `sources`. For example, `sources = ["rtl/**/*.sv"]` uses `rtl` as the default include directory. When `sources` is omitted, Vizsla does not infer include directories from best-effort indexing. If `include_dirs = []` is set explicitly, no fallback is used.

`libraries` are loaded as dependency workspaces and participate in the current project's analysis. `exclude` is a workspace-relative glob that filters generated files, simulation output, or black-box files out of loaded files. See [Project Configuration](./project-configuration.md#paths-and-globs) for glob syntax.

## Why Diagnostics Differ

Single-file parse diagnostics only need the current file, for example a missing semicolon or unmatched parenthesis. Cross-file semantic diagnostics need more project information: include directories, predefined macros, library paths, and which files belong to the same project.

Common outcomes:

| File state | Single-file parse diagnostics | Cross-file semantic diagnostics |
| --- | --- | --- |
| `.v` or `.sv` files loaded through explicit `sources` | Can run | Can run when project analysis is available |
| Headers found through `include_dirs` | Participate through the source file that includes them | Participate through the including source file's project analysis |
| Files only found through best-effort indexing | Usually opened files only | Not run |
| Files filtered by `exclude` | Not run | Not run |

Header files (`.vh`, `.svh`, `.svi`) are usually not standalone compile entries. They mainly participate after a `.v` or `.sv` file includes them. Opening a header directly, or only listing its directory in `include_dirs`, does not mean Vizsla will run full standalone diagnostics for that header.

## Navigation and Duplicate Modules

Go to definition, references, hover, completion, and code lens prefer information from loaded indexes. Best-effort indexing makes these features available early, but it is not a strict compile configuration.

In project analysis, duplicate module names are handled through the project view. Vizsla does not treat directory names as implicit namespaces.

In best-effort indexing, if several modules with the same name are visible, Vizsla makes an editor-only nearest-candidate guess: same file first, then deepest shared directory, then same scan root. The guess is used only when there is one best candidate; ties stay ambiguous.

This guess is not a SystemVerilog language rule. If there is one nearest candidate, Vizsla does not report a diagnostic. If no unique candidate exists, Vizsla reports `ambiguous-module-instantiation` as information. In configured projects, duplicate module names are still handled by stricter semantic rules; when slang semantic diagnostics are enabled, Vizsla prefers slang's diagnostics.

## Qihe Project Analysis

Automatic Qihe project analysis currently requires `vizsla.toml` in the working directory. If only legacy `vizsla_config.toml` exists, normal VS Code features still read it for compatibility, but Qihe falls back to single-file input.

When `vizsla.toml` exists, Qihe uses the compile plan from project analysis. Vizsla only passes project files, `--top`, `-I`, and `-D` arguments when that plan has real source files.

If the current file only comes from best-effort indexing, or if no project compile plan is available, Vizsla lets Qihe fall back to single-file input. This prevents default indexing from accidentally triggering project analysis.
