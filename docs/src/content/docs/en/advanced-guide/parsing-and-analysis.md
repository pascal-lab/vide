---
title: "Indexing, Project Configuration, and Diagnostics"
description: How Vide loads files, builds project analysis, produces diagnostics, and runs Qihe in configured and unconfigured workspaces.
---

This page explains Vide's analysis boundaries: why some navigation and completion features work without `vide.toml`, and why diagnostics, rename, and Qihe become more complete after `vide.toml` is configured.

For field syntax, see [Configure the First Project](../../user-guide/first-project/) and [Project Configuration Reference](../../user-guide/project-configuration/). This page only explains what those fields change.

## Three Working States

Vide enters different working states depending on whether the workspace has `vide.toml` and whether `sources` is configured explicitly:

| State | What gets loaded | What it is for |
| --- | --- | --- |
| No `vide.toml` | Verilog/SystemVerilog files in the workspace are scanned into a best-effort index | Initial code reading: basic definition jumps, references, hover, and completion |
| `vide.toml` exists but omits `sources` | Best-effort indexing continues; if `include_dirs` is configured, those directories are loaded as include search paths | Transitional state; not recommended as a long-term setup |
| `sources = []` | Workspace source scanning is explicitly disabled; if `include_dirs` is configured, only those include directories are loaded | Newly created templates, or workspaces where Vide should not guess the source layout |
| `sources = ["rtl/**"]` | Project source files are loaded from `sources`, then combined with `include_dirs`, `defines`, `libraries`, and `top_modules` to build project analysis | Normal project configuration |

A short way to remember it:

- Omitted `sources`: scan the workspace for code reading, but do not treat the scan result as the configured project.
- `sources = []`: do not scan source files automatically.
- `sources = ["rtl/**"]`: these files are the current project sources.

## Best-Effort Indexing and Project Analysis

Best-effort indexing is for reading code. It tries to load RTL files in the workspace so definition jumps, references, hover, completion, and the instance-count lens can work early. It is not a real compile configuration, and it does not enable full project semantic diagnostics or project rename.

Project analysis comes from `vide.toml`. After `sources` points to actual source files, Vide adds those files to the project view and uses `include_dirs`, `defines`, `libraries`, and `top_modules` for cross-file parsing, diagnostics, rename, and Qihe project analysis.

`libraries` are loaded as dependency workspaces and participate in the current project analysis. `exclude` removes generated files, simulation output, or black-box files from already loaded files. See [Project Configuration Reference](../../user-guide/project-configuration/#path-and-glob-rules-for-sources-and-exclude) for path and glob syntax.

## How Include Directories Participate

`include_dirs` are include search paths. They do not mean every file in those directories is an independent compile entry.

Header files (`.vh`, `.svh`, `.svi`) usually participate after a `.v` or `.sv` file includes them. Opening a header directly, or listing its directory in `include_dirs`, does not mean Vide runs full standalone project diagnostics for that header.

If `sources` is explicit but `include_dirs` is omitted, Vide infers default include directories from `sources`. For example, `sources = ["rtl/**/*.sv"]` uses `rtl` as a default include directory. Setting `include_dirs = []` explicitly disables that fallback.

## Why Diagnostics Differ

Diagnostics have two layers:

- Single-file parse diagnostics: only need the current file, such as syntax errors, unmatched brackets, or parse failures.
- Cross-file semantic diagnostics: need project information, such as target modules, include directories, macro branches, libraries, and top modules.

Common behavior:

| File state | Parse diagnostics | Cross-file semantic diagnostics |
| --- | --- | --- |
| Source file explicitly loaded by `sources` | Available | Available |
| Header found through `include_dirs` | Participates through the source file that includes it | Participates through the including source file's project analysis |
| File only found through best-effort indexing | Usually only for opened files | Not used as a project diagnostic entry |
| File filtered by `exclude` | Not run | Not run |

So it is normal to see basic diagnostics in an unconfigured workspace. For cross-file semantic diagnostics, configure `vide.toml` first.

## Navigation and Duplicate Modules

Definition jumps, references, hover, completion, and the instance-count lens prefer already loaded index information. Best-effort indexing lets those features work in unconfigured workspaces, but it can only make editor-level guesses.

In project analysis, duplicate module names are handled through the current project view. Vide does not treat directory names as implicit namespaces; if several duplicate module names are visible, the project should resolve that ambiguity through project configuration, library boundaries, or build scripts.

In best-effort indexing, if one instance can match several modules with the same name, Vide makes a nearest-candidate guess for code-reading features only: same file first, then deepest shared directory, then same scan root. The guess is used only when there is one best candidate; ties stay ambiguous.

This guess is not a SystemVerilog language rule. If there is one nearest candidate, Vide does not report a diagnostic. If no unique candidate exists, Vide reports an informational `ambiguous-module-instantiation` diagnostic. Configured projects still use stricter semantic rules; when third-party `slang` semantic diagnostics are enabled, Vide prefers slang's diagnostics.

## When Qihe Runs as a Project

Qihe integration reuses Vide's project configuration discovery result. If the working directory has a usable `vide.toml` and the compile plan contains actual source files, Vide passes project files, `--top`, `-I`, and `-D` arguments.

If the current file only comes from best-effort indexing, or if the project configuration does not produce a usable compile plan, Vide falls back to single-file Qihe input. This avoids treating unconfigured scan results as a real project configuration.

For Qihe command shapes, settings, and failure handling, see [Qihe Analysis](../../user-guide/features/qihe/).
