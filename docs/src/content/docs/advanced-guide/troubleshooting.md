---
title: 高级故障排查
description: 排查本地 VSIX、扩展自带服务器、自定义服务器、文件监听、日志和 profiling 问题。
---

本页只保留偏高级启动、日志和调试链路的问题。普通用户遇到诊断不更新、跳不到定义、格式化失败或 Qihe 运行失败时，优先看 [日常使用](../../user-guide/daily-use/) 中对应功能页。

启动链路先按 [当扩展无法正常启动](../check-server/) 自检；命令、状态栏和输出通道入口见 [操作参考](../commands-status-logs/)。

## 本地 VSIX 安装后找不到服务器

扩展默认在自己的安装目录下寻找 `server/vizsla.exe` 或 `server/vizsla`。本地打包或调试 VSIX 时，只运行 `npm run compile` 不会生成服务器二进制，也不会把它复制进扩展目录。

可以在 `editors/vscode` 下打包 debug VSIX：

```powershell
npm run package:debug
```

也可以直接配置本地服务器：

```json
{
  "vizsla.server.command": "D:\\Proj\\vizsla\\target\\release\\vizsla.exe"
}
```

保存后选择提示里的 `重启`，或执行 `Vide：重启语言服务器`。

## 自定义启动命令、参数或工作目录启动失败

检查这些点：

- `vizsla.server.command` 使用绝对路径，并能在终端执行 `--version`。
- `vizsla.server.args` 和 `vizsla.server.additionalArgs` 都是字符串数组。
- `vizsla.server.cwd` 如果设置，必须是已存在目录。
- 修改 `vizsla.server.command`、`vizsla.server.args`、`vizsla.server.additionalArgs`、`vizsla.server.cwd` 或 `vizsla.trace.server` 后，接受扩展提示里的 `重启`。

示例：

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.args": [],
  "vizsla.server.cwd": "D:\\work\\chip",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

完整字段说明见 [Server 设置](../vscode-settings/#server)。

## 状态栏显示启动错误

点击 `Vide` 状态项打开状态菜单，选择显示输出，或执行 `Vide：显示语言服务器输出`。优先看这些输出：

- `Bundled Vide Language Server binary not found`
- `Unsupported platform-architecture combination`
- `Failed to start language server`
- `Server command`
- `Server args`
- `Working directory`

如果错误来自项目配置，转到 [项目配置](../../user-guide/project-configuration/) 修正工作区根目录下的 `vizsla.toml`。如果错误来自服务器启动命令，继续按本页检查自定义服务器或 VSIX 包。

## 文件变化没有触发刷新

默认 `vizsla.files.watcher` 是 `client`，会优先使用 VS Code 的文件变化通知。客户端不支持动态监听文件时，会回退到服务端监听。

如果工程文件变化后没有触发刷新：

```json
{
  "vizsla.files.watcher": "server"
}
```

`vizsla.files.excludeDirs` 只接受工作区相对目录，不支持 glob。文件选择请优先使用项目配置文件的 `sources` / `exclude` 路径模式；如果还要减少 VS Code 自己的文件监听事件，另配 VS Code 的 `files.watcherExclude`。

## 需要更详细的服务器日志

如果语言服务器能启动，但需要更详细的内部日志，在 `vizsla.server.additionalArgs` 中添加 `--log` 和 `--log_file`，然后重启语言服务器：

```json
{
  "vizsla.server.additionalArgs": ["--log", "debug", "--log_file", "D:\\work\\vide-server.log"]
}
```

如果连启动都失败，先不要加复杂参数；按 [启动自检](../check-server/) 确认服务器路径、平台和 `--version`。

## 诊断性能分析产物异常

`Vide：分析诊断性能` 会启动独立的临时语言服务器，不会复用当前编辑会话。产物目录、trace、summary 和 flamegraph 路径会写到 `Vide Profiling` 输出通道。

如果没有产物：

- 确认当前工作区或当前文件能被正常分析。
- 打开 `Vide Profiling` 输出通道查看临时服务器启动错误。
- 临时服务器仍然使用当前 `vizsla.server.command`、`vizsla.server.args` 和相关配置；自定义服务器错误会影响 profiling。

产物格式说明见 [操作参考](../commands-status-logs/#高级诊断性能分析产物)。
