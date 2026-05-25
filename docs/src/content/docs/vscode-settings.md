---
title: VS Code 设置参考
description: Vizsla VS Code 扩展的紧凑配置项参考。
---

所有设置都在 `vizsla.*` 命名空间下。可以在 VS Code Settings UI 搜索 `Vizsla`，也可以直接编辑 `settings.json`。

## 常用设置速查

大多数用户只会改这些：

| 你想做什么 | 常用设置 |
| --- | --- |
| 让 VS Code 调用本机的 Qihe | `vizsla.qihe.command` |
| 指定本机的 `verible-verilog-format` | `vizsla.formatter.path` |
| 控制诊断刷新是保存后还是输入时 | `vizsla.diagnostics.update` |
| 开关端口、参数、结构结尾的行内提示 | `vizsla.inlayHints.*` |
| 开关模块声明上方的实例数量提示 | `vizsla.lens.instantiations.enable` |
| 项目配置变化后是否自动刷新 | `vizsla.workspace.auto.reload` |

服务器启动命令、文件监听、诊断规则和通信跟踪更偏排障或开发场景；不确定时保持默认值。

## Server

这组设置用于替换或调试后台语言服务器。普通安装通常不需要改。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.server.command` | `null` | 自定义语言服务器命令。留空时使用扩展自带服务器。 |
| `vizsla.server.args` | `[]` | 启动服务器时传入的前置参数。 |
| `vizsla.server.additionalArgs` | `[]` | 启动服务器时追加的参数，常用于 `--log` / `--log_file`。 |
| `vizsla.server.cwd` | `null` | 服务器工作目录。默认使用第一个工作区目录；没有工作区时使用扩展目录。 |
| `vizsla.trace.server` | `"off"` | LSP 通信跟踪。可选 `"off"`、`"messages"`、`"verbose"`。 |

这些服务器启动设置变更后，扩展会提示 `重启`：`vizsla.server.command`、`vizsla.server.args`、`vizsla.server.additionalArgs`、`vizsla.server.cwd`、`vizsla.trace.server`。

示例：

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

## Qihe

如果你不用 `Vizsla：运行 Qihe 分析`，这组设置可以保持默认值。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.qihe.command` | `"qihe"` | 调用 Qihe 的命令。必须在 VS Code 可见的 `PATH` 中，也可以写绝对路径。 |
| `vizsla.qihe.autoConfigureArgsFromManifest` | `true` | 根据 `vizsla.toml` 自动添加 Qihe 编译模式和转发给 slang 的选项。 |
| `vizsla.qihe.compileArgs` | `[]` | 插入到 `qihe compile` 之后的参数，用于手动选择编译模式或转发 slang 选项。 |
| `vizsla.qihe.runArgs` | `["-g", "std"]` | 通过 `Vizsla：运行 Qihe 分析` 运行 `qihe run` 时追加的参数。 |

`Vizsla：运行 Qihe 分析` 只对本地 Verilog/SystemVerilog 文件可用。默认会从当前 `vizsla.toml` 推导 Qihe 编译模式、顶层模块、include 目录和宏定义；如果项目已经用脚本管理这些参数，关闭自动推导并显式配置 `compileArgs` / `runArgs`。

示例：

```json
{
  "vizsla.qihe.command": "D:\\tools\\qihe\\qihe.exe",
  "vizsla.qihe.autoConfigureArgsFromManifest": false,
  "vizsla.qihe.compileArgs": ["--mode", "sv", "--", "-I", "include"],
  "vizsla.qihe.runArgs": ["-g", "std"]
}
```

## Files

这组设置主要用于文件监听排障。选择哪些 RTL 文件属于项目，优先写在 `vizsla.toml` 的 `sources` / `exclude` 中。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.files.excludeDirs` | `[]` | 工作区相对目录排除列表。不支持 glob；文件选择 glob 写在项目配置文件的 `sources` / `exclude` 中。 |
| `vizsla.files.watcher` | `"client"` | 文件监听方式。可选 `"client"`、`"notify"`、`"server"`。 |

`client` 会优先使用 VS Code 文件变化通知。客户端不支持动态监听文件时会回退到服务端监听；`notify` 和 `server` 都走服务端监听路径。

## Workspace

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.workspace.auto.reload` | `true` | 项目配置文件变更后自动刷新工程信息。 |

## Scope

这组设置会影响跳转、引用、重命名等阅读能力；不确定时保持默认值。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.scope.visibility` | `"private"` | 控制作用域内符号对其它作用域的可见性。可选 `"private"`、`"public"`。 |

这个设置会影响引用搜索、重命名和当前文件高亮。

## Formatter 和 Formatting

只有需要格式化 Verilog/SystemVerilog 时才需要配置这组。Vizsla 不自带 `verible-verilog-format`。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.formatter.provider` | `"verible"` | 格式化后端。当前支持 `verible`，会调用外部 `verible-verilog-format`。 |
| `vizsla.formatter.path` | `null` | `verible` 格式化后端使用的可执行文件路径。留空时查找 `verible-verilog-format`。 |
| `vizsla.formatter.args` | `["--failsafe_success=false"]` | 传给 `verible-verilog-format` 的参数。 |
| `vizsla.formatting.on.enter` | `true` | 按 Enter 时启用格式化行为。 |
| `vizsla.formatting.in.comments` | `true` | 在注释内启用 Enter 辅助格式化。 |
| `vizsla.formatting.indent.width` | `4` | 编辑器没有提供 formatting options 时使用的后备缩进宽度。 |

`verible` 格式化后端会在自定义参数后追加当前缩进宽度对应的 `--indentation_spaces=<N>`。

## Inlay Hints

行内提示会直接显示在编辑器里，适合日常阅读端口和参数连接。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.inlayHints.port.connection.enable` | `true` | 显示端口连接行内提示。 |
| `vizsla.inlayHints.parameter.assignment.enable` | `true` | 显示参数赋值行内提示。 |
| `vizsla.inlayHints.end.structure.enable` | `true` | 显示结构结束名行内提示。 |

## Lens

实例数量提示会显示在模块声明上方。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.lens.instantiations.enable` | `true` | 显示模块实例数量提示。 |

## Semantic Tokens

语义高亮会在主题支持时让端口方向、时钟复位和读写位置更容易区分。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.semantic.tokens.port.clk.rst.enable` | `true` | 为时钟/复位端口启用专用语义高亮标记。 |
| `vizsla.semantic.tokens.port.input.output.enable` | `true` | 为输入/输出端口启用专用语义高亮标记。 |

## Diagnostics

诊断会显示在 VS Code 的 `Problems` 面板和编辑器下划线里。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.diagnostics.enable` | `true` | 启用所有 Vizsla 诊断。 |
| `vizsla.diagnostics.update` | `"onSave"` | 诊断刷新时机。可选 `"onSave"`、`"onType"`。 |
| `vizsla.diagnostics.parse.enable` | `true` | 启用单文件语法诊断。 |
| `vizsla.diagnostics.semantic.enable` | `true` | 启用需要项目信息的跨文件诊断。 |
| `vizsla.diagnostics.slang.warnings` | `[]` | slang warning 选项，例如 `default`、`everything`、`none`、`error`、`no-<name>`、`error=<name>`。 |
| `vizsla.diagnostics.slang.rules` | `[]` | 诊断过滤或严重程度覆盖规则。 |

`vizsla.diagnostics.slang.warnings` 对齐 slang `-W...` 语义，但在 VS Code 设置里不写前导 `-W`。`vizsla.diagnostics.slang.rules` selector 支持 `code:<subsystem>:<code>`、`option:<name>`、`group:<name>`、`source:parse`、`source:semantic`；`severity` 可选 `ignore`、`info`、`warning`、`error`、`fatal`。

示例：

```json
{
  "vizsla.diagnostics.slang.rules": [
    { "selector": "source:parse", "severity": "warning" },
    { "selector": "option:unconnected-port", "severity": "ignore" }
  ]
}
```

## Signature Help

签名帮助用于实例端口连接和参数赋值列表。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.signature.help.params.only` | `false` | 只显示参数相关签名帮助。 |
