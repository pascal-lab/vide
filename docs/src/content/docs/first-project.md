---
title: 第一个工程
description: 为 Vizsla 准备第一个 Verilog/SystemVerilog 工程目录。
---

## 推荐目录

小工程可以先用简单结构:

```text
my-rtl/
  rtl/
    top.sv
    alu.sv
  include/
    defs.svh
```

直接用 VS Code 打开 `my-rtl`:

```powershell
code D:\work\my-rtl
```

## 没有清单时会怎样

如果 VS Code 打开的 workspace root 下有 Verilog/SystemVerilog 文件, 且没有 `vizsla.toml` 或旧版 `vizsla_config.toml`, 扩展会创建默认 `vizsla.toml` 并弹出提示:

```toml
# Syntax-only startup config. Keep these arrays empty to avoid scanning the workspace.
# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.
sources = []
include_dirs = []
```

这个默认清单不会扫描 workspace root, 只运行打开文件的 syntax/parse diagnostics, 不运行 semantic diagnostics。这适合你刚打开陌生工程时快速获得低成本语法反馈。你可以之后再按实际目录写入 `sources` 或 `include_dirs`, 并按需补充 `defines`, `libraries` 或 `top_modules`, 来启用语义诊断和跨文件能力。

如果通过其它客户端或命令行方式启动服务器, 且确实没有 `vizsla.toml` 或 `vizsla_config.toml`, Vizsla 只保留 syntax/parse diagnostics, 不运行 semantic diagnostics。

## 什么时候创建清单

当工程变大时, 我们建议编辑 workspace root 下的 `vizsla.toml`:

- 你只想扫描 `rtl` 和 `include`, 不想扫描仿真输出、生成目录或第三方缓存。
- 你需要设置 `defines`。
- 你需要把 include 目录和 source 目录分开。
- 你有外部库目录, 希望它们作为依赖参与分析。
- 你想显式声明 `top_modules`。

示例:

```toml
top_modules = ["top"]
defines = ["SYNTHESIS", "DATA_WIDTH=32"]
sources = ["rtl"]
include_dirs = ["include"]
exclude = ["build", "out"]
```

清单只会从你打开的 workspace root 读取。我们不会自动向父目录或子目录搜索其它清单; 如果 `vizsla.toml` 和 `vizsla_config.toml` 同时存在, 会优先读取 `vizsla.toml`。
