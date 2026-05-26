---
title: VS Code 设置参考
description: Vide VS Code 扩展的紧凑配置项参考。
---

所有设置都在 `vide.*` 命名空间下。可以在 VS Code 设置界面搜索 `Vide`，也可以直接编辑 `settings.json`。

从日常任务进入： [语言识别](../../user-guide/daily-use/language-support/) / [诊断](../../user-guide/daily-use/diagnostics/) / [导航和阅读](../../user-guide/daily-use/navigation/) / [补全与快速修复](../../user-guide/daily-use/editing-assistance/) / [格式化](../../user-guide/daily-use/formatting/) / [结构辅助](../../user-guide/daily-use/structure/) / [Qihe](../../user-guide/daily-use/qihe/)。

## 常用设置速查

大多数用户只会改这些：

| 你想做什么 | 常用设置 |
| --- | --- |
| 让 VS Code 调用本机的 Qihe | `vide.qihe.command` |
| 指定本机的 `verible-verilog-format` | `vide.formatter.path` |
| 控制诊断刷新是保存后还是输入时 | `vide.diagnostics.update` |
| 开关端口、参数、结构结尾的行内提示 | `vide.inlayHints.*` |
| 开关模块声明上方的实例数量提示 | `vide.lens.instantiations.enable` |
| 项目配置变化后是否自动刷新 | `vide.workspace.auto.reload` |

服务器启动命令、文件监听、诊断规则和通信跟踪更偏排障或开发场景；日常使用保持默认值即可。

## Server

这组设置用于替换或调试后台语言服务器。日常使用保持默认值即可。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.server.command` | `null` | 自定义语言服务器命令。留空时使用扩展自带服务器。 |
| `vide.server.args` | `[]` | 启动服务器时传入的前置参数。 |
| `vide.server.additionalArgs` | `[]` | 启动服务器时追加的参数，常用于 `--log` / `--log_file`。 |
| `vide.server.cwd` | `null` | 服务器工作目录。默认使用第一个工作区目录；没有工作区时使用扩展目录。 |
| `vide.trace.server` | `"off"` | LSP 通信跟踪。可选 `"off"`、`"messages"`、`"verbose"`。 |

这些服务器启动设置变更后，扩展会提示 `重启`：`vide.server.command`、`vide.server.args`、`vide.server.additionalArgs`、`vide.server.cwd`、`vide.trace.server`。

示例：

```json
{
  "vide.server.command": "D:\\tools\\vide\\vide.exe",
  "vide.server.additionalArgs": ["--log", "debug"]
}
```

## Qihe

运行 `Vide：运行 Qihe 分析` 时才需要看这组设置。

对应日常使用页：[Qihe](../../user-guide/daily-use/qihe/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.qihe.command` | `"qihe"` | 调用 Qihe 的命令。必须在 VS Code 可见的 `PATH` 中，也可以写绝对路径。 |
| `vide.qihe.autoConfigureArgsFromManifest` | `true` | 根据当前项目配置文件自动添加 Qihe 编译模式和转发给 slang 的选项。 |
| `vide.qihe.compileArgs` | `[]` | 插入到 `qihe compile` 之后的参数，用于手动选择编译模式或转发 slang 选项。 |
| `vide.qihe.runArgs` | `["-g", "std"]` | 通过 `Vide：运行 Qihe 分析` 执行 `qihe run` 时追加的参数。 |

`Vide：运行 Qihe 分析` 只对本地 Verilog/SystemVerilog 文件可用。默认会从当前 `vide.toml` 推导 Qihe 编译模式、顶层模块、include 目录和宏定义。如果项目已经用脚本管理这些参数，关闭自动推导并显式配置 `compileArgs` / `runArgs`。

示例：

```json
{
  "vide.qihe.command": "D:\\tools\\qihe\\qihe.exe",
  "vide.qihe.autoConfigureArgsFromManifest": false,
  "vide.qihe.compileArgs": ["--mode", "sv", "--", "-I", "include"],
  "vide.qihe.runArgs": ["-g", "std"]
}
```

## Files

这组设置主要用于文件监听排障。选择哪些 RTL 文件属于项目，优先写在 `vide.toml` 的 `sources` / `exclude` 中。

对应日常使用页：[语言识别](../../user-guide/daily-use/language-support/)；项目文件选择见 [项目配置](../../user-guide/project-configuration/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.files.excludeDirs` | `[]` | 工作区相对目录排除列表。不支持 glob；文件选择 glob 写在项目配置文件的 `sources` / `exclude` 中。 |
| `vide.files.watcher` | `"client"` | 文件监听方式。可选 `"client"`、`"notify"`、`"server"`。 |

`client` 会优先使用 VS Code 文件变化通知。客户端不支持动态监听文件时会回退到服务端监听；`notify` 和 `server` 都走服务端监听路径。

## Workspace

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.workspace.auto.reload` | `true` | 项目配置文件变更后自动刷新工程信息。 |

## Scope

这组设置会影响跳转、引用、重命名等阅读能力；日常使用保持默认值即可。

对应日常使用页：[导航和阅读](../../user-guide/daily-use/navigation/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.scope.visibility` | `"private"` | 控制作用域内符号对其它作用域的可见性。可选 `"private"`、`"public"`。 |

这个设置会影响引用搜索、重命名和当前文件高亮。

## Formatter 和 Formatting

配置格式化时看这组。Vide 会调用外部 `verible-verilog-format`。

对应日常使用页：[格式化](../../user-guide/daily-use/formatting/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.formatter.provider` | `"verible"` | 格式化后端。当前支持 `verible`，会调用外部 `verible-verilog-format`。 |
| `vide.formatter.path` | `null` | `verible` 格式化后端使用的可执行文件路径。留空时查找 `verible-verilog-format`。 |
| `vide.formatter.args` | `["--failsafe_success=false"]` | 传给 `verible-verilog-format` 的参数。 |
| `vide.formatting.on.enter` | `true` | 按 Enter 时启用格式化行为。 |
| `vide.formatting.in.comments` | `true` | 在注释内启用 Enter 辅助格式化。 |
| `vide.formatting.indent.width` | `4` | 编辑器没有提供格式化选项时使用的后备缩进宽度。 |

`verible` 格式化后端会在自定义参数后追加当前缩进宽度对应的 `--indentation_spaces=<N>`。

## Inlay Hints

行内提示会直接显示在编辑器里，适合日常阅读端口和参数连接。

对应日常使用页：[结构辅助](../../user-guide/daily-use/structure/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.inlayHints.port.connection.enable` | `true` | 显示端口连接行内提示。 |
| `vide.inlayHints.parameter.assignment.enable` | `true` | 显示参数赋值行内提示。 |
| `vide.inlayHints.end.structure.enable` | `true` | 显示结构结束名行内提示。 |

## Lens

实例数量提示会显示在模块声明上方。

对应日常使用页：[结构辅助](../../user-guide/daily-use/structure/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.lens.instantiations.enable` | `true` | 显示模块实例数量提示。 |

## Semantic Tokens

语义高亮会在主题支持时让端口方向、时钟复位和读写位置更容易区分。

对应日常使用页：[语言识别](../../user-guide/daily-use/language-support/) 和 [结构辅助](../../user-guide/daily-use/structure/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.semantic.tokens.port.clk.rst.enable` | `true` | 为时钟/复位端口启用专用语义高亮标记。 |
| `vide.semantic.tokens.port.input.output.enable` | `true` | 为输入/输出端口启用专用语义高亮标记。 |

## Diagnostics

诊断会显示在 VS Code 的 `Problems` 面板和编辑器下划线里。

对应日常使用页：[诊断](../../user-guide/daily-use/diagnostics/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.diagnostics.enable` | `true` | 启用所有 Vide 诊断。 |
| `vide.diagnostics.update` | `"onSave"` | 诊断刷新时机。可选 `"onSave"`、`"onType"`。 |
| `vide.diagnostics.parse.enable` | `true` | 启用单文件语法诊断。 |
| `vide.diagnostics.semantic.enable` | `true` | 启用需要项目信息的跨文件诊断。 |
| `vide.diagnostics.slang.warnings` | `[]` | slang warning 选项，例如 `default`、`everything`、`none`、`error`、`no-<name>`、`error=<name>`。 |
| `vide.diagnostics.slang.rules` | `[]` | 诊断过滤或严重程度覆盖规则。 |

`vide.diagnostics.slang.warnings` 对齐 slang `-W...` 语义，但在 VS Code 设置里不写前导 `-W`。`vide.diagnostics.slang.rules` 的选择器支持 `code:<subsystem>:<code>`、`option:<name>`、`group:<name>`、`source:parse`、`source:semantic`；`severity` 可选 `ignore`、`info`、`warning`、`error`、`fatal`。

示例：

```json
{
  "vide.diagnostics.slang.rules": [
    { "selector": "source:parse", "severity": "warning" },
    { "selector": "option:unconnected-port", "severity": "ignore" }
  ]
}
```

## Signature Help

签名帮助用于实例端口连接和参数赋值列表。

对应日常使用页：[补全与快速修复](../../user-guide/daily-use/editing-assistance/)。

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vide.signature.help.params.only` | `false` | 只显示参数相关签名帮助。 |
