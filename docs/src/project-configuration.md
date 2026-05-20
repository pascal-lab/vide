# 项目配置

Vizsla 的工程清单文件名固定为 `vizsla_config.toml`。它必须放在 workspace root 下, 或者被显式作为工程路径传入。字段名必须使用当前支持的名字, 旧字段或未知字段会被拒绝。

## 完整示例

```toml
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

所有路径都相对于 `vizsla_config.toml` 所在目录解析。

VS Code 在缺少清单时会生成 syntax-only 默认清单:

```toml
# Syntax-only startup config. Keep these empty arrays to avoid scanning the workspace.
# Do not delete them unless you want omitted fields to default to the workspace root.
# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.
sources = []
include_dirs = []
```

这个默认清单不扫描工程目录, 也不建立编译 profile; 打开的文件仍会获得 syntax/parse diagnostics。需要 semantic diagnostics 和跨文件能力时, 请把 `sources`, `include_dirs`, `defines`, `libraries` 或 `top_modules` 改成符合工程结构的实际配置。空文件不是这个默认清单; 如果手动保留空的 `vizsla_config.toml`, 字段会按省略规则处理。

## 字段说明

| 字段 | 类型 | 作用 |
| --- | --- | --- |
| `top_modules` | 字符串数组 | 声明当前工程的顶层模块名。我们会把它写入编译 profile。 |
| `defines` | 字符串数组 | 预定义宏。支持 `"NAME"` 和 `"NAME=value"` 两种形式。 |
| `sources` | 路径数组或省略 | 源文件扫描根目录。省略时默认使用 workspace root。显式写成 `[]` 时不会回退到 workspace root。 |
| `include_dirs` | 路径数组或省略 | 预处理 include 搜索目录。省略时默认等于 `sources`。显式写成 `[]` 时不会回退。 |
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

- 如果库目录下有 `vizsla_config.toml`, 我们会按这个清单加载库。
- 如果库目录没有清单, 我们会把该目录作为未配置库加载。
- 依赖会参与当前工程的编译 profile, 并支持传递依赖。

## exclude

`exclude` 适合排除生成目录、仿真输出目录、缓存目录。它只接受路径, 不是 glob:

```toml
exclude = ["build", "out", "sim/work"]
```

如果你还希望 VS Code 自己减少文件监听, 可以同时配置 VS Code 的 `files.watcherExclude`。Vizsla 的 `exclude` 和 VS Code 的 watcher 设置是两套机制。

## 常见注意事项

- 清单文件名必须是 `vizsla_config.toml`。
- `top_module`, `include`, `macros` 这类旧字段不会被接受。
- 我们不会从子目录自动发现清单。打开 `repo` 时, 只检查 `repo/vizsla_config.toml`。
- 配置了 `sources = []` 的工程可以只加载 include 目录, 适合头文件型 workspace。
