---
title: 操作参考：命令、状态和日志
description: Vizsla 命令、状态栏项和输出通道的速查。
---

这页是操作入口参考。服务器是否正常启动请按 [服务器自检流程](./check-server.md) 检查；具体故障请按 [故障排查](./troubleshooting.md) 里的症状处理。

## 命令面板命令

VS Code 扩展贡献这些命令。日常使用时看“命令面板标题”即可；命令 ID 主要给快捷键绑定或脚本使用。

| 命令 ID | 命令面板标题 | 用途 |
| --- | --- | --- |
| `vizsla.showOutput` | `Vizsla：显示语言服务器输出` | 打开 `Vizsla Language Server` 输出通道。 |
| `vizsla.restartServer` | `Vizsla：重启语言服务器` | 停止并重新启动当前语言服务器。 |
| `vizsla.showServerVersion` | `Vizsla：显示服务器版本` | 使用当前服务器命令、工作目录和环境，将 `vizsla.server.args` 与 `--version` 组合执行版本查询；不会附加 `vizsla.server.additionalArgs`。 |
| `vizsla.reloadWorkspace` | `Vizsla：重新加载项目配置` | 不重启服务器，重新读取项目配置并刷新工程信息。 |
| `vizsla.showStatus` | `Vizsla：显示状态` | 打开 Vizsla 状态菜单。 |
| `vizsla.runQiheAnalysis` | `Vizsla：运行 Qihe 分析` | 对当前本地 Verilog/SystemVerilog 文件运行 Qihe 分析。 |
| `vizsla.profileDiagnostics` | `Vizsla：分析诊断性能` | 启动临时语言服务器并对工作区或当前文件执行一次诊断性能分析。 |

`vizsla.runQiheAnalysis` 只对本地文件可用，且文件扩展名必须是 `.v`、`.vh`、`.sv`、`.svh` 或 `.svi`。

## 状态栏

主状态项名为 `Vizsla`，位于 VS Code 右侧状态栏。它用于显示语言服务器和项目配置状态：

| 状态 | 含义 |
| --- | --- |
| 普通 `Vizsla` 文本 | 服务器已启动；悬停可查看项目配置状态。 |
| spinner | 服务器启动、停止或项目配置加载进行中。 |
| warning 图标 | 通常表示当前 workspace 缺少项目配置文件。 |
| error 图标 | 服务器启动失败或项目配置加载失败。 |

点击 `Vizsla` 状态项或执行 `Vizsla：显示状态` 会打开状态菜单。菜单会显示 `打开项目配置文件`、`创建项目配置文件`、`分析诊断性能`、`重新加载项目`、`重启语言服务器`、`显示输出` 等操作；具体显示哪些取决于当前项目状态。

执行 Qihe 分析时还会出现独立的 `Qihe` 状态项。运行失败时，点击这个状态项会打开 `Vizsla Qihe` 输出通道。

## 输出通道

| 输出通道 | 记录内容 |
| --- | --- |
| `Vizsla Language Server` | 扩展激活、平台、VS Code 版本、服务器命令、参数、工作目录、扩展自带服务器查找结果、启动、停止、重启和版本查询结果。 |
| `Vizsla Qihe` | `vizsla.runQiheAnalysis` 的目标文件、命令进度、Qihe 输出和失败信息。 |
| `Vizsla Profiling` | `vizsla.profileDiagnostics` 的目标、产物目录、diagnostics 请求耗时和生成文件路径。 |

## 高级：诊断性能分析产物

`Vizsla：分析诊断性能` 面向大型工程排障。它会生成：

| 文件 | 说明 |
| --- | --- |
| `trace.json` | Chrome/Perfetto/Speedscope 兼容 trace；扩展内置 Speedscope viewer 会打开这个文件。 |
| `summary.json` | 请求耗时、diagnostics 汇总和 top span 汇总。 |
| `trace.folded` | 从 trace 生成的 folded stack。 |
| `flamegraph.svg` | 静态火焰图。 |
| `server.log` | 临时语言服务器日志。 |

诊断性能分析会使用独立的临时语言服务器进程，不会重启或影响编辑器当前使用的语言服务器。

## 服务器启动设置变更

这些设置变更后，扩展会提示 `重启`：

- `vizsla.server.command`
- `vizsla.server.args`
- `vizsla.server.additionalArgs`
- `vizsla.server.cwd`
- `vizsla.trace.server`

可以选择提示里的 `重启`，或手动执行 `Vizsla：重启语言服务器`。
