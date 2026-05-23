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
| `Vizsla: Profile Diagnostics` | 对工作区或当前 Verilog/SystemVerilog 文件运行一次独立 diagnostics profiling, 并生成 trace、summary 和 flamegraph。 |

## 状态栏

扩展会在状态栏左侧显示服务器状态:

| 状态 | 含义 |
| --- | --- |
| `Vizsla Starting` | 正在创建并启动语言服务器。 |
| `Vizsla Ready` | 语言服务器已经启动。 |
| `Vizsla Stopping` | 正在停止语言服务器。 |
| `Vizsla Stopped` | 语言服务器已停止。 |
| `Vizsla Error` | 服务器启动失败。 |

点击状态栏项会打开输出通道。出现 `Vizsla Error` 时, 先看这里。

## 输出通道

扩展输出通道名称是 `Vizsla Language Server`。这里会记录:

- 扩展激活信息。
- 扩展安装路径。
- 当前平台和架构。
- VS Code 版本。
- 服务器命令、参数和工作目录。
- bundled server 查找结果。
- 启动、停止、重启和版本查询结果。

执行 `Vizsla: Profile Diagnostics` 时, 扩展还会打开 `Vizsla Profiling` 输出通道。这里会显示本次 profiling 的目标、产物目录、诊断请求耗时和生成的文件路径。

## 性能分析诊断

执行 `Vizsla: Profile Diagnostics`。扩展会启动一个独立的临时语言服务器进程, 然后根据选择的目标发送一次诊断请求:

- 工作区目标发送 `workspace/diagnostic`, 用于观察项目级诊断路径。
- 当前文件目标发送 `textDocument/diagnostic`, 用于缩小到单文件诊断路径。

请求结束后扩展会关闭临时进程; 这个过程不会重启或影响正在使用的语言服务器。

完成后会生成:

| 文件 | 说明 |
| --- | --- |
| `trace.json` | Chrome/Perfetto/speedscope 兼容 trace。 |
| `summary.json` | 请求耗时、diagnostics 汇总和 top span 汇总。 |
| `trace.folded` | 从 trace 生成的 folded stack。 |
| `flamegraph.html` | 可点击缩放和搜索的交互式火焰图。 |
| `flamegraph.svg` | 静态火焰图备用文件。 |
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
