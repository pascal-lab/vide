---
title: 命令、状态栏和日志
description: Vizsla 的命令面板命令、状态栏提示和输出通道。
---

## 命令面板命令

VS Code 扩展贡献了这些命令:

| 命令 | 作用 |
| --- | --- |
| `Vizsla: Show Language Server Output` | 打开 `Vizsla Language Server` 输出通道。 |
| `Vizsla: Restart Language Server` | 停止并重新启动语言服务器。 |
| `Vizsla: Show Server Version` | 执行服务器 `--version`, 并显示第一行版本输出。 |
| `Vizsla: Reload Project Configuration` | 不重启语言服务器, 重新读取项目配置文件并刷新工程信息。 |
| `Vizsla: Show Status` | 打开 Vizsla 状态菜单。 |
| `Vizsla: Run Qihe Analysis` | 对当前 Verilog/SystemVerilog 文件运行 Qihe 分析。 |
| `Vizsla: Profile Diagnostics` | 对工作区或当前 Verilog/SystemVerilog 文件运行一次独立 diagnostics profiling, 并生成 trace、summary 和 flamegraph。 |

## 状态栏

扩展会在 VS Code 右侧状态栏显示名为 `Vizsla` 的状态项。启动、停止或加载项目配置时会带旋转图标, 缺少项目配置文件时会带 warning 图标, 服务器启动失败或项目配置加载失败时会带 error 图标。将鼠标悬停在状态项上可以查看当前详情, 例如服务器是否运行、项目配置是否已加载、是否没有项目配置文件, 或项目配置是否失败。

点击状态栏项或执行 `Vizsla: Show Status` 会打开状态菜单。菜单会在顶部显示项目配置错误, 并提供这些操作:

- 打开已有 `vizsla.toml`。
- 为缺少配置的 workspace 创建 `vizsla.toml`。
- 运行 diagnostics profiling。
- 重新加载项目配置。
- 重启语言服务器。
- 打开 `Vizsla Language Server` 输出通道。

执行 Qihe 分析时还会出现独立的 `Qihe` 状态项, 用于显示运行中、完成或失败状态。Qihe 失败时点击该状态项会打开 `Vizsla Qihe` 输出通道。

## 输出通道

`Vizsla Language Server` 会记录:

- 扩展激活信息。
- 扩展安装路径。
- 当前平台和架构。
- VS Code 版本。
- 服务器命令、参数和工作目录。
- bundled server 查找结果。
- 启动、停止、重启和版本查询结果。

`Vizsla Qihe` 会记录 `Vizsla: Run Qihe Analysis` 的目标文件、命令进度、Qihe 输出和失败信息。Qihe 运行失败时, 错误通知里的 `Show Qihe Output` 会打开这个输出通道。

执行 `Vizsla: Profile Diagnostics` 时, 扩展还会打开 `Vizsla Profiling` 输出通道。这里会显示本次 profiling 的目标、产物目录、诊断请求耗时和生成的文件路径。

## 运行 Qihe 分析

打开 Verilog/SystemVerilog 文件后执行 `Vizsla: Run Qihe Analysis`。扩展会把请求发给当前语言服务器, 并根据 `vizsla.qihe.*` 设置调用 Qihe。Qihe 命令、compile 参数、run 参数和自动从项目配置补全参数的行为请看 [VS Code 设置](./vscode-settings.md#qihe)。

## 性能分析诊断

执行 `Vizsla: Profile Diagnostics`。扩展会启动一个独立的临时语言服务器进程, 然后根据选择的目标发送一次诊断请求:

- 工作区目标发送 `workspace/diagnostic`, 用于观察项目级诊断路径。
- 当前文件目标发送 `textDocument/diagnostic`, 用于缩小到单文件诊断路径。

请求结束后扩展会关闭临时进程; 这个过程不会重启或影响正在使用的语言服务器。

完成后会生成:

| 文件 | 说明 |
| --- | --- |
| `trace.json` | Chrome/Perfetto/Speedscope 兼容 trace, 也就是 Speedscope 的交互式输入文件。 |
| `summary.json` | 请求耗时、diagnostics 汇总和 top span 汇总。 |
| `trace.folded` | 从 trace 生成的 folded stack。 |
| `flamegraph.svg` | 静态火焰图备用文件。交互式查看会在 VS Code 标签页中用扩展内置的本地 Speedscope viewer 打开 `trace.json`。 |
| `server.log` | 临时语言服务器日志。 |

## 查询服务器版本

你可以从命令面板执行 `Vizsla: Show Server Version`。扩展会解析当前服务器启动配置, 然后执行:

```powershell
vizsla --version
```

如果配置了 `vizsla.server.command`, 版本查询会使用这个自定义命令。扩展会把 `vizsla.server.args` 放在 `--version` 前面。

## 配置变更后重启

这些启动相关设置变更后, 扩展会提示你重启语言服务器:

- `vizsla.server.command`
- `vizsla.server.args`
- `vizsla.server.additionalArgs`
- `vizsla.server.cwd`
- `vizsla.trace.server`

选择提示里的 `Restart`, 或手动执行 `Vizsla: Restart Language Server`。
