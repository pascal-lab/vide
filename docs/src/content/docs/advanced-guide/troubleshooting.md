---
title: 高级故障排查
description: 按症状排查本地 VSIX、自定义服务器、文件监听、日志和 profiling 问题。
---

本页处理已经超出普通功能使用的问题，例如本地 VSIX、替换服务器、文件监听、服务器日志和诊断性能分析。诊断、跳转、格式化或 Qihe 的普通使用问题，优先回到 [功能特性](../../user-guide/features/) 中对应功能页。

如果你还不能确认语言服务器是否启动，先看 [当扩展无法正常启动](../check-server/)。命令、状态栏和输出通道名称见 [命令、状态和日志](../../user-guide/commands-status-logs/)。

## 先按症状分流

| 症状 | 先看哪里 | 常见原因 |
| --- | --- | --- |
| 状态栏显示语言服务器错误 | `Vide Language Server` 输出通道 | VSIX 平台不匹配、自定义命令不存在、工作目录错误 |
| 本地 VSIX 安装后找不到服务器 | 本页“本地 VSIX 找不到服务器” | 只编译了扩展，没有把服务器二进制打进 VSIX |
| 自定义服务器在终端能跑，在扩展里失败 | 本页“自定义服务器启动失败” | `vide.server.cwd`、参数数组、VS Code 进程 PATH 和终端不同 |
| 改文件后没有刷新 | 本页“文件变化没有触发刷新” | 文件 watcher 没收到事件，或文件被排除 |
| 需要内部日志或性能数据 | 本页“服务器日志”和“诊断性能分析” | 需要额外启动参数或 profiling 产物 |

## 本地 VSIX 找不到服务器

扩展默认在自己的安装目录下寻找 `server/vide.exe` 或 `server/vide`。本地调试时，如果只运行 `npm run compile`，只会生成扩展 JavaScript，不会构建服务器，也不会把服务器复制进扩展目录。

要生成包含服务器的本地 VSIX，在 `editors/vscode` 下运行：

```powershell
npm run package:debug
```

如果只是想让已安装扩展使用本地构建的服务器，可以改用自定义服务器路径：

```json
{
  "vide.server.command": "D:/Proj/vizsla/target/release/vide.exe"
}
```

保存后选择提示里的 `重启`，或执行 `Vide：重启语言服务器`。完整构建流程见 [从源码构建安装](../advanced-installation/#build-from-source-installation)。

## 自定义服务器启动失败

先确认扩展实际使用的命令。打开 `Vide Language Server` 输出通道，找到 `Server command`、`Server args` 和 `Working directory`。

然后检查：

- `vide.server.command` 使用绝对路径。
- 这个命令能在终端执行 `--version`。
- `vide.server.args` 和 `vide.server.additionalArgs` 都是字符串数组。
- `vide.server.cwd` 如果设置，必须是已经存在的目录。
- 修改 `vide.server.command`、`vide.server.args`、`vide.server.additionalArgs`、`vide.server.cwd` 或 `vide.trace.server` 后，需要重启语言服务器。

示例：

```json
{
  "vide.server.command": "D:/tools/vide/vide.exe",
  "vide.server.args": [],
  "vide.server.cwd": "D:/work/chip",
  "vide.server.additionalArgs": ["--log", "debug"]
}
```

完整字段说明见 [Server 设置](../../user-guide/vscode-settings/#server)。

## 状态栏显示项目配置错误

状态栏的错误不一定来自语言服务器启动。点击 `Vide` 状态项并打开输出后，先区分错误来源：

- 出现 `Bundled Vide Language Server binary not found`、`Unsupported platform-architecture combination`、`Failed to start language server`：继续检查 VSIX 或自定义服务器。
- 出现 `failed to load workspace`、`manifest ...`、`vide.toml` 相关错误：这是项目配置错误。

项目配置错误应该回到 [配置第一个项目](../../user-guide/first-project/) 或 [项目配置参考](../../user-guide/project-configuration/) 修正工作区根目录下的 `vide.toml`。

## 文件变化没有触发刷新

默认 `vide.files.watcher` 是 `client`，会优先使用 VS Code 的文件变化通知。客户端不支持动态监听文件时，Vide 会回退到服务端监听。

如果工程文件变化后没有触发刷新，可以先临时切到服务端监听：

```json
{
  "vide.files.watcher": "server"
}
```

`vide.files.excludeDirs` 只接受工作区相对目录，不支持 glob。项目文件选择请优先使用 `vide.toml` 的 `sources` 和 `exclude`；如果还要减少 VS Code 自己的文件监听事件，再配置 VS Code 的 `files.watcherExclude`。

## 打开更详细的服务器日志

如果语言服务器能启动，但需要看内部日志，在 `vide.server.additionalArgs` 中添加 `--log` 和 `--log_file`，然后重启语言服务器：

```json
{
  "vide.server.additionalArgs": ["--log", "debug", "--log_file", "D:/work/vide-server.log"]
}
```

如果服务器本身还启动不了，先不要加复杂日志参数；先看 [当扩展无法正常启动](../check-server/) 确认服务器路径、平台和 `--version`。

## 诊断性能分析没有产物

`Vide：分析诊断性能` 会启动独立的临时语言服务器，不会复用当前编辑会话。产物目录、trace、summary 和 flamegraph 路径会写到 `Vide Profiling` 输出通道。

如果没有产物：

- 确认当前工作区或当前文件能被正常分析。
- 打开 `Vide Profiling` 输出通道查看临时服务器启动错误。
- 临时服务器仍然使用当前 `vide.server.command`、`vide.server.args` 和相关配置；自定义服务器错误会影响 profiling。

产物格式说明见 [命令、状态和日志](../../user-guide/commands-status-logs/#高级诊断性能分析产物)。
