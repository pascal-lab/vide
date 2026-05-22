---
title: Daily Use
description: Diagnostics, navigation, completion, formatting, and code actions provided by Vizsla in VS Code.
---

This chapter introduces Vizsla by IDE workflow. Each feature is described in terms of what you can see, when it triggers, and which settings affect it, so you can verify the behavior in VS Code feature by feature.

## Language Detection and Syntax Highlighting

The VS Code extension registers two languages:

| Language | File extensions |
| --- | --- |
| Verilog | `.v`, `.vh` |
| SystemVerilog | `.sv`, `.svh`, `.svi` |

After opening one of these files, VS Code applies the TextMate grammar and language configuration provided by Vizsla. Syntax highlighting does not depend on a successfully started language server. Even while the server is still starting, you should see basic highlighting, comment rules, and bracket matching.

After the language server starts, it provides basic language services. Semantic features such as cross-file diagnostics, navigation, completion, semantic highlighting, inlay hints, and code lens require enough project information from the project manifest.

## Diagnostics

Vizsla diagnostics have two layers:

- Parse diagnostics: syntax problems found during parsing.
- Semantic diagnostics: compile or semantic problems such as instance, port, parameter, type, and reference errors.

The defaults are:

- `vizsla.diagnostics.enable`: `true`
- `vizsla.diagnostics.parse.enable`: `true`
- `vizsla.diagnostics.semantic.enable`: `true`
- `vizsla.diagnostics.update`: `onSave`

Diagnostics refresh on save by default. For large RTL projects, we recommend keeping `onSave` so heavier semantic refreshes do not run after every keystroke. If you want live diagnostics while editing, use:

```json
{
  "vizsla.diagnostics.update": "onType"
}
```

Diagnostics appear in the VS Code `Problems` panel and as editor underlines. Some quick fixes read stable data attached to diagnostics, so you usually need to see the relevant diagnostic before the corresponding code action appears.

Semantic diagnostics require a loadable project configuration in `vizsla.toml` under the workspace root, such as real `sources` shell globs or `include_dirs`; `defines`, `libraries`, and `top_modules` can add more project information. The legacy `vizsla_config.toml` still works, but `vizsla.toml` wins when both files exist. If the manifest is missing or `sources` is omitted, Vizsla indexes the workspace root by default for read-only features such as navigation and references, but it does not run cross-file semantic diagnostics. Setting `sources = []` explicitly disables workspace indexing.

slang warning settings live under `vizsla.diagnostics.slang.*`. For warning names, warning groups, and `-W...` semantics, see the Diagnostics section in [VS Code Settings](./vscode-settings.md#diagnostics).

## Go to Definition and Declaration

Vizsla provides both `Go to Definition` and `Go to Declaration`. In daily RTL reading, you can use them to:

- Jump from an instantiation to the target module definition.
- Jump from a signal reference to its declaration.
- Jump from names such as ports, parameters, typedefs, functions, and tasks to the matching definition or declaration.
- Navigate within one file or across files, as long as those files are loaded by the current project.

`Go to Declaration` falls back to definition logic when it cannot find a better declaration. When VS Code supports location links, Vizsla returns fuller source and target ranges; otherwise it returns normal locations.

If the navigation result is unexpected, first check whether the project loaded the target file. Default indexing without a manifest is best-effort. In workspaces with duplicate module names, generated directories, or third-party libraries mixed in, it may return a result that does not match the actual compile configuration. For more complex layouts, configure `sources`, `include_dirs`, and `libraries` explicitly.

## Find References and Document Highlights

`Find References` searches symbol references based on semantic parsing, which is more suitable for RTL code reading than plain text search. Document highlights are the local version: when the cursor is on a symbol, VS Code can highlight related references in the current file.

Both features are affected by `vizsla.scope.visibility`:

| Setting | Behavior |
| --- | --- |
| `private` | Default. Symbols inside a scope are not exposed to other scopes by default, except ports. |
| `public` | Relaxes scope visibility so references and rename search a wider range. |

If references for local variables, generate blocks, or named blocks are too broad or too narrow, check this setting first.

## Rename

Vizsla supports `Prepare Rename` and `Rename Symbol`. VS Code asks the server whether the current position can be renamed before performing the rename, which prevents accidental rename attempts on keywords, literals, or unstable positions.

Rename generates a workspace edit from reference search, so it depends on the same semantic project information as `Find References`. Before renaming, we recommend checking that:

- All target files have been loaded by the project.
- The current symbol can be navigated or searched correctly.
- `scope.visibility` matches your project conventions.

## Completion

Completion triggers at common Verilog/SystemVerilog input points. The server currently declares these trigger characters:

```text
. ( , @ # ` ' newline
```

Vizsla chooses completion sources based on the current position:

- Preprocessor directives: completes `define`, `include`, `ifdef`, `ifndef`, `elsif`, `pragma`, `timescale`, `default_nettype`, and related items after a backtick.
- Keywords and snippets: provides suitable keywords and snippets in contexts such as module items, procedural statements, generate blocks, specify blocks, config blocks, and library maps.
- Expression candidates: completes currently visible values in assignment right-hand sides, conditional expressions, procedural statements, function/task arguments, and similar positions.
- Member access: completes struct members, hierarchical members, or resolvable member names after `.`.
- Ports and parameters: completes candidates in named connections `.port(...)`, named parameters `#(.PARAM(...))`, and ordered connection positions.
- Sensitivity lists: completes signals and event keywords after `@` or in event-control contexts.
- System tasks/functions: completes slang-provided system subroutine facts such as `$display` and `$bits` in expression or statement contexts.

Completion avoids unrelated candidates inside comments, strings, and literals where possible. For instance port and parameter completion, the project must be able to resolve the instantiated target module.

## Snippets

Vizsla includes a set of Verilog/SystemVerilog snippets. They appear together with normal keyword completions, but snippet edits are only returned when the VS Code client declares snippet support.

Common snippets include:

- Top-level declarations: `module`, `primitive`, `macromodule`, `config`.
- Library maps: `library`, `include`.
- Parameter lists: `parameter`, `localparam`.
- Module items: `wire`, `reg`, `genvar`, `generate`, `function`, `task`, `assign`, `always`, `initial`.
- Control statements: `if`, `ifelse`, `case`, `casez`, `casex`, `for`, `while`, `repeat`, `forever`, `wait`.
- Preprocessor directives: `define`, `include`, `ifdef`, `ifndef`, `elsif`, `pragma`, `timescale`, `default_nettype`.

These snippets are not a simple global list. Vizsla filters them by syntax context. For example, `module` is more suitable at the top level, `parameter` appears in parameter port lists or module items, and procedural statement snippets are not offered arbitrarily at the top level.

## Hover

Hover first identifies names and literals:

- For symbol names, Vizsla resolves the definition and renders information about modules, ports, parameters, declarations, instances, functions, and other items.
- For port connection shorthand, Vizsla shows information for both the port side and the local side.
- For literals, Vizsla renders parsed literal information.

When VS Code supports Markdown hover, Vizsla returns Markdown; otherwise it returns plain text. Hover information depends on whether the semantic definition at the current position can be resolved. If the project is not loaded completely, the information may be reduced.

## Signature Help

Signature help triggers on `(`, `,`, and `.` and mainly serves two scenarios:

- Module instance port connections: shows the target module port list and marks the active parameter based on the current position.
- Parameter assignment lists: shows the target module parameter list.

By default, signatures include port or parameter type/declaration information where possible. If you only want parameter-related content, enable:

```json
{
  "vizsla.signature.help.params.only": true
}
```

Signature help also depends on resolving the target module. If the target module is missing, dependencies are not loaded, or include/define configuration is incomplete, signature help may not appear.

## Formatting

Vizsla supports three formatting entry points:

- `Format Document`
- `Format Selection`
- On-type formatting when pressing Enter

The default formatter provider is `verible`, which calls the external `verible-verilog-format` executable. If it is not on `PATH`, configure:

```json
{
  "vizsla.formatter.path": "D:\\tools\\verible\\verible-verilog-format.exe"
}
```

`vizsla.formatter.args` is passed to `verible-verilog-format`. The server also appends `--indentation_spaces=<N>` based on the editor-provided `tabSize`.

Enter-key formatting does more than call the formatter. Vizsla also handles comment continuation and previous-line structure formatting, controlled by:

- `vizsla.formatting.on.enter`
- `vizsla.formatting.in.comments`
- `vizsla.formatting.indent.width`

If you only want to disable Enter behavior without affecting manual `Format Document`, disable `vizsla.formatting.on.enter`.

## Code Actions

Current code actions focus on module instances, port connections, and parameter assignment fixes. Fix actions usually appear as quick fixes after a relevant diagnostic appears, and conversion actions can also appear as refactors.

| Action | Purpose |
| --- | --- |
| `Fill connections` | Fill missing port connections. Named connections add `.name()`, while ordered connections try to use available same-name signals or placeholder expressions. |
| `Fill parameters` | Fill missing parameter assignments. Named parameters add `.PARAM(...)`, while ordered parameters follow the target parameter order. |
| `Convert ordered port connections to named connections` | Rewrite ordered port connections into named port connections. |
| `Convert ordered parameter assignments to named assignments` | Rewrite ordered parameter assignments into named parameter assignments. |
| `Remove empty port connections` | Remove redundant empty connections from a named port list, such as a trailing extra comma. |
| `Add explicit empty port connection` | Add explicit empty parentheses for an implicit empty port connection. |
| `Add empty instance port list` | Add `()` to an instance that has no port list. |

These actions reduce mechanical RTL editing. Vizsla generates edits from the parsed target module port and parameter order, so the instance target must be resolvable.

## Semantic Highlighting

Vizsla provides semantic tokens. Compared with TextMate highlighting, semantic tokens come from semantic analysis and can distinguish more RTL roles.

Current port highlighting has two configurable enhancements:

- `vizsla.semantic.tokens.port.clk.rst.enable`: marks 1-bit clock/reset style ports with dedicated semantic token modifiers. Clock names match `clock`, `clk`, and `tck`; reset names match common `reset` / `rst` forms.
- `vizsla.semantic.tokens.port.input.output.enable`: marks ports by direction with read/write/ref modifiers. `input` maps to read, `output` maps to write, `inout` maps to read + write, and `ref` maps to ref.

If your theme supports the corresponding semantic token types and modifiers, clock/reset ports, input/output ports, and ordinary symbols are easier to distinguish.

The VS Code extension contributes a default italic style for semantic tokens with the `read` modifier, matching common `input` ports. It contributes a default bold style for semantic tokens with the `write` modifier, matching common `output` ports.

## Folding and Outline

Vizsla supports folding ranges and document symbols.

Folding ranges cover common structures such as modules, blocks, statements, and comments. VS Code displays these as foldable regions so you can collapse long modules, generate blocks, case statements, or long comments.

Document symbols populate the VS Code Outline view. Vizsla collects symbols such as modules, configs, UDPs, libraries, ports, parameters, net/data declarations, typedefs, instances, blocks, functions, generate blocks, and specify blocks from HIR/source maps. When the client supports hierarchical document symbols, Outline keeps the hierarchy; otherwise Vizsla returns flat symbol information.

## Selection Range

Vizsla supports selection ranges. You can use VS Code's expand/shrink selection commands to grow the current token selection into an expression, statement, block, or larger syntax structure.

This is useful when selecting an RTL structure before refactoring, or when quickly selecting the current subexpression inside a complex expression.

## Inlay Hints

Three inlay hint categories are enabled by default:

| Setting | Default | Description |
| --- | --- | --- |
| `vizsla.inlayHints.port.connection.enable` | `true` | Shows target port names for ordered port connections or empty connections. |
| `vizsla.inlayHints.parameter.assignment.enable` | `true` | Shows target parameter names for ordered parameter assignments. |
| `vizsla.inlayHints.end.structure.enable` | `true` | Shows structure names at the end of module structures. |

Port and parameter hints mainly improve readability of RTL instantiations. For example, ordered connection `u(a, b, c)` requires checking the target module port list, while hints can show labels such as `clk:`, `rst_n:`, and `data:` directly.

For ordered connections and parameter assignments, hints also carry target locations and optional text edits. Clients that support these capabilities can use hints as navigation or quick conversion entry points.

## Instance Code Lens

Module instance code lens is enabled by default:

```json
{
  "vizsla.lens.instantiations.enable": true
}
```

Vizsla shows the instance count at module declarations. Parsed titles look like `0 instances`, `1 instance`, or `N instances`. The count comes from whole-project reference search and helps you understand whether a module is instantiated and at what scale.

The current code lens only shows the count and does not bind a navigation command. Use `Find References` when you need to locate specific instances.
