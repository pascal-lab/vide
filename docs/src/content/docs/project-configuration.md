---
title: 项目配置
description: 配置 vizsla.toml 工程清单、源文件、include 目录和宏定义。
---

Vizsla 的工程清单文件名优先使用 `vizsla.toml`。旧文件名 `vizsla_config.toml` 仍然兼容, 但当两个文件同时存在时会读取 `vizsla.toml`。清单必须放在 workspace root 下, 或者被显式作为工程路径传入。字段名必须使用当前支持的名字, 旧字段或未知字段会被拒绝。

## 完整示例

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/vizsla.schema.json

top_modules = ["top"]

defines = [
  "SYNTHESIS",
  "DATA_WIDTH=32",
  "RESET_VALUE=0",
]

sources = [
  "rtl",
  "ip/local_ip/src",
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
  "build",
  "sim/work",
  "generated/tmp",
]
```

所有路径都相对于工程清单所在目录解析。

VS Code 在包含 Verilog/SystemVerilog 文件的 workspace 缺少清单时会生成 syntax-only 默认 `vizsla.toml`:

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/vizsla.schema.json
# Syntax-only startup config. Keep these arrays empty to avoid scanning the workspace.
# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.
sources = []
include_dirs = []
```

这个默认清单不扫描工程目录, 也不建立编译 profile; 打开的文件仍会获得 syntax/parse diagnostics。空的清单和省略 `sources` 的清单也不会扫描 workspace root。需要 semantic diagnostics 和跨文件能力时, 请写入符合工程结构的 `sources` 或 `include_dirs`, 并按需补充 `defines`, `libraries` 或 `top_modules`。

编辑清单时, TOML 结构诊断、字段补全、hover 和格式化交给 Tombi。Vizsla 只读取清单来构建工程模型, 并在清单变更后刷新工程信息。

## Tombi Schema

推荐安装 [Tombi](https://github.com/tombi-toml/tombi) 来编辑 `vizsla.toml`。Tombi 可以用 JSON Schema 提供 TOML 结构诊断、字段补全、hover 和格式化。Vizsla 扩展生成的默认清单会在文件顶部加入 schema directive:

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/vizsla.schema.json
```

这个 directive 是 TOML 注释, 没有安装 Tombi 时也不会影响 Vizsla 读取清单。已有清单可以手动把它加到文件开头。

## 字段说明

| 字段 | 类型 | 作用 |
| --- | --- | --- |
| `top_modules` | 字符串数组 | 声明当前工程的顶层模块名。我们会把它写入编译 profile。 |
| `defines` | 字符串数组 | 预定义宏。支持 `"NAME"` 和 `"NAME=value"` 两种形式。 |
| `sources` | 路径数组或省略 | 源文件扫描根目录。省略时等同于 `[]`, 不会扫描 workspace root。 |
| `include_dirs` | 路径数组或省略 | 预处理 include 搜索目录。省略时默认等于最终的 `sources`。显式写成 `[]` 时不会回退。 |
| `libraries` | 路径数组 | 外部库或依赖工程。我们会把它们作为 library workspace 加载。 |
| `exclude` | 路径数组 | 从 `sources`, `include_dirs`, `libraries` 中排除的目录。 |

## sources 和 include_dirs

`sources` 决定我们扫描哪些 Verilog/SystemVerilog 文件。当前工程加载器读取这些扩展名:

- `.v`
- `.sv`
- `.vh`
- `.svh`
- `.svi`
- `.map`

`include_dirs` 会进入预处理配置, 用于处理 include 搜索。没有显式配置 `include_dirs` 时, 我们会使用最终的 `sources` 作为 include 目录。

## defines

`defines` 中每一项都是一个宏定义:

```toml
defines = [
  "SYNTHESIS",
  "WIDTH=32",
  "MODE=fast sim",
]
```

宏名需要是合法标识符。带值宏会按 `NAME=value` 解析, `value` 可以包含空格。

## libraries

`libraries` 用于声明依赖库。每个库路径会被当作工程路径继续解析:

- 如果库目录下有 `vizsla.toml`, 我们会按这个清单加载库; 否则回退到 `vizsla_config.toml`。
- 如果库目录没有清单, 我们会把该目录作为未配置库加载。
- 依赖会参与当前工程的编译 profile, 并支持传递依赖。

## exclude

`exclude` 适合排除生成目录、仿真输出目录、缓存目录。它只接受路径, 不是 glob:

```toml
exclude = ["build", "out", "sim/work"]
```

如果你还希望 VS Code 自己减少文件监听, 可以同时配置 VS Code 的 `files.watcherExclude`。Vizsla 的 `exclude` 和 VS Code 的 watcher 设置是两套机制。

## 常见注意事项

- 推荐清单文件名是 `vizsla.toml`; `vizsla_config.toml` 仅作为兼容旧工程的 fallback。
- `top_module`, `include`, `macros` 这类旧字段不会被接受。
- 我们不会从子目录自动发现清单。打开 `repo` 时, 只检查 `repo/vizsla.toml`, 再回退检查 `repo/vizsla_config.toml`。
- 配置了 `sources = []` 的工程可以只加载 include 目录, 适合头文件型 workspace。
