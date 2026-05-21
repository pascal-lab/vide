---
title: 项目配置
description: 配置 vizsla.toml 工程清单、源文件、include 目录和宏定义。
---

Vizsla 的工程清单文件名优先使用 `vizsla.toml`。旧文件名 `vizsla_config.toml` 仍然兼容, 但当两个文件同时存在时会读取 `vizsla.toml`。清单必须放在 workspace root 下, 或者被显式作为工程路径传入。字段名必须使用当前支持的名字, 旧字段或未知字段会被拒绝。

## 完整示例

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

`sources` 和 `exclude` 使用 workspace-relative shell glob 语义, 统一用 `/` 作为路径分隔符。`include_dirs` 和 `libraries` 仍然是相对于工程清单所在目录解析的路径。

VS Code 在包含 Verilog/SystemVerilog 文件的 workspace 缺少清单时会生成默认 `vizsla.toml`:

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/v1/vizsla.schema.json
# Default startup manifest. Omitting sources enables best-effort indexing for navigation
# without semantic diagnostics. Fill shell globs, for example sources = ["rtl/**"]
# and include_dirs = ["include"], to enable semantic diagnostics.
# Set sources = [] to disable workspace indexing.
```

这个默认清单会让 Vizsla 以 best-effort 方式索引 workspace 下的 Verilog/SystemVerilog 文件, 用于跳转、引用等读能力, 但不会建立编译 profile 或运行跨文件 semantic diagnostics。显式写入 `sources = []` 会关闭 workspace 索引, 回到只处理打开文件的低成本模式。需要更准确的 semantic diagnostics 时, 请写入符合工程结构的 `sources` shell glob 或 `include_dirs`, 并按需补充 `defines`, `libraries` 或 `top_modules`。

Vizsla 只读取清单来构建工程模型, 并在清单变更后刷新工程信息。文件开头的 schema directive 是 TOML 注释, 不会影响 Vizsla 读取清单。schema URL 带有版本段; 后续清单格式变化时, 新版 Vizsla 可以生成指向新版 schema 的 directive, 已有工程仍保留原来的 schema。

如果你想确认省略 `sources`、显式 `sources = []`、解析、索引和诊断之间的关系, 可以阅读 [解析与分析模型](./parsing-and-analysis.md)。

## 字段说明

| 字段 | 类型 | 作用 |
| --- | --- | --- |
| `top_modules` | 字符串数组 | 声明当前工程的顶层模块名。我们会把它写入编译 profile。 |
| `defines` | 字符串数组 | 预定义宏。支持 `"NAME"` 和 `"NAME=value"` 两种形式。 |
| `sources` | shell glob 数组或省略 | 源文件选择模式。省略时会默认索引 workspace root, 但不启用 semantic diagnostics; 显式写成 `[]` 时关闭 workspace 索引。 |
| `include_dirs` | 路径数组或省略 | 预处理 include 搜索目录。省略时默认等于 `sources` 推导出的扫描根目录。显式写成 `[]` 时不会回退。 |
| `libraries` | 路径数组 | 外部库或依赖工程。我们会把它们作为 library workspace 加载。 |
| `exclude` | shell glob 数组 | 从已加载文件中排除匹配的源文件或头文件。 |

## sources 和 include_dirs

`sources` 决定我们扫描哪些 Verilog/SystemVerilog 文件。它使用 shell glob 语义, 不再把同一个字符串同时解释为路径和 pattern。常用写法:

```toml
sources = ["rtl/**", "ip/**/*.sv", "tb/**/*.sv"]
```

`*`, `?`, `[]`, `{}` 和 `**` 按 shell glob 处理; `*` 不跨 `/`, 递归目录请写 `**`。pattern 必须相对 workspace root, 不能使用绝对路径、`..` 或反斜杠。当前工程加载器读取这些扩展名:

- `.v`
- `.sv`
- `.vh`
- `.svh`
- `.svi`
- `.map`

`include_dirs` 会进入预处理配置, 用于处理 include 搜索。没有显式配置 `include_dirs` 时, 我们会使用 `sources` pattern 推导出的扫描根目录作为 include 目录。例如 `sources = ["rtl/**/*.sv"]` 会把 `rtl` 作为默认 include 目录。显式写成 `include_dirs = []` 时不会回退。

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

`exclude` 使用同一套 workspace-relative shell glob 语义, 适合排除生成目录、仿真输出目录、缓存目录或黑盒 stub:

```toml
exclude = ["build/**", "out/**", "sim/work/**", "**/*_bb.v"]
```

`exclude = ["build"]` 只匹配名为 `build` 的路径本身, 不递归排除目录内容; 目录递归请写 `build/**`。如果你还希望 VS Code 自己减少文件监听, 可以同时配置 VS Code 的 `files.watcherExclude`。Vizsla 的 `exclude` 和 VS Code 的 watcher 设置是两套机制。

## 常见注意事项

- 推荐清单文件名是 `vizsla.toml`; `vizsla_config.toml` 仅作为兼容旧工程的 fallback。
- `top_module`, `include`, `macros` 这类旧字段不会被接受。
- 我们不会从子目录自动发现清单。打开 `repo` 时, 只检查 `repo/vizsla.toml`, 再回退检查 `repo/vizsla_config.toml`。
- 配置了 `sources = []` 的工程可以只加载 include 目录, 适合头文件型 workspace。
