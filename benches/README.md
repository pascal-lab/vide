# Vide comparison benches

This is the **product** harness: user-visible LSP latency, slang compiler
ceiling, and accuracy against slang-server. It is **not** wired into CI.

Investigation leftovers that used to live as `#[ignore]` tests in `ide` were
deleted. Do not add more `Instant::now` + `println!` benches there.

## Workloads

| name | size | upstream |
| --- | --- | --- |
| `common_cells` | small | [pulp-platform/common_cells](https://github.com/pulp-platform/common_cells) |
| `ibex` | medium | [lowRISC/ibex](https://github.com/lowRISC/ibex) |
| `cva6` | large | [openhwgroup/cva6](https://github.com/openhwgroup/cva6) |

They are optional submodules (`update = none`). A normal clone does not fetch
them. Init only what you want:

```text
git submodule update --init benches/workloads/common_cells
git submodule update --init benches/workloads/ibex
git submodule update --init benches/workloads/cva6
```

`vide.toml`, slang-server flags, and probe coordinates are tracked under
`benches/overlays/<name>/`. The harness copies them into the tree for the run
and removes them afterwards. Do not commit those copies into the submodule.

## Servers

On `PATH`, or override with env:

| role | binary | env |
| --- | --- | --- |
| Vide | `target/release/vide` (built if missing) | `VIDE_BIN` |
| slang-server | `slang-server` | `SLANG_SERVER_BIN` |
| Verible LS | `verible-verilog-ls` | `VERIBLE_LS_BIN` |
| svls | `svls` | `SVLS_BIN` |
| slang compiler | `slang` | `SLANG_BIN` |

Missing competitors are reported as `N/A`, not a hard failure.

slang-server is the accuracy oracle (same frontend family as Vide, different
IDE). The `slang` binary is a compile-time ceiling, not an LSP.

Cited common_cells numbers in the design-unit-graph work used slang-server
**0.2.10+c1e0b0c** (`SLANG_SERVER_BIN` / `PATH`). The repo does not pin that
version; record the binary you compared against when publishing a result.

## Run

```text
cargo xtask bench
cargo xtask bench --workload common_cells
cargo xtask bench --server vide --server slang-server
```

Writes `benches/results/<timestamp>.json` and `.md`.
