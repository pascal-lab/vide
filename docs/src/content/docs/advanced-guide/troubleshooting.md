---
title: 故障报告与排查
description: 报告 Vide 故障，并按常见症状处理启动和文件刷新问题。
---

有时 Vide 可能出现故障。

有些故障是 Vide 本身的问题，这种情况欢迎通过 [GitHub Issues](https://github.com/pascal-lab/vide/issues) 向我们报告，并尽量附上可复现例子。
有些故障来自工作区或扩展配置，这种情况可以先按本页的常见问题和应对处理。

普通功能使用问题，优先回到 [功能特性](../../user-guide/features/) 的对应页面。命令、状态栏含义和输出通道名称见 [命令、状态和日志](../../user-guide/commands-status-logs/)。

## 故障报告

如果出现功能异常，欢迎在 [GitHub Issues](https://github.com/pascal-lab/vide/issues) 向我们报告，并尽量附上：

- 触发问题的最小例子
- 期望行为和实际行为
- 使用的平台、VS Code 版本，以及 `Vide：显示服务器版本` 看到的扩展版本和服务器版本
- 能稳定复现时的操作步骤

建议先执行 `Vide：显示服务器版本`，再执行 `Vide：显示语言服务器输出`，把 `Vide Language Server` 输出通道里的相关内容一起附到 issue。

`Vide Language Server` 输出通道通常已经包含排查最需要的信息，例如：

- 扩展激活和当前平台
- VS Code 版本
- 实际使用的服务器命令、参数和工作目录
- `Vide：显示服务器版本` 查询结果
- 启动失败或退出时的错误

如果 output 里的信息还不够，或者问题只会在较长流程里出现，再打开更详细的文件日志。在 `vide.server.additionalArgs` 中加入 `--log` 和 `--log_file`，然后重启语言服务器：

```json
{
  "vide.server.additionalArgs": ["--log", "debug", "--log_file", "D:/work/vide-server.log"]
}
```

如果服务器本身还启动不了，可以先看下面的“扩展或自定义服务器无法启动”，先确认扩展实际使用的服务器命令能正常运行。

## 常见问题和应对

### 状态栏提示语言服务器错误

先打开 `Vide Language Server` 输出通道，重点看最后一条错误，以及是否出现：

```text
[INFO] Language server started successfully
```

常见问题是：

- `Bundled Vide Language Server binary not found` 或 `Unsupported platform-architecture combination`：
  先核对安装的 VSIX 和当前平台是否匹配。本地打包时，只有 `npm run package:*` 或 `npm run package:debug` 会把服务器打进 VSIX；单独执行 `npm run compile` 只会编译扩展前端。
- `Failed to start language server`、自定义命令不存在、无执行权限：
  继续看下面的“扩展或自定义服务器无法启动”。
- 状态栏只是提示 `vide.toml`、`manifest` 或 `failed to load workspace`：
  这通常不是启动问题，应该回到工作区根目录检查 `vide.toml`。相关说明见 [配置第一个项目](../../user-guide/first-project/) 和 [项目配置参考](../../user-guide/project-configuration/)。

完整的本地打包和安装流程见 [从源码构建安装](../advanced-installation/#build-from-source-installation)。

### 扩展或自定义服务器无法启动

先确认扩展实际使用的命令。点击状态栏的 `Vide`，或执行 `Vide：显示状态` 和 `Vide：显示语言服务器输出`，记录下面几项：

- `Platform`
- `Server command`
- `Server args`
- `Working directory`

然后执行 `Vide：显示服务器版本`。如果这个命令也失败，说明扩展当前使用的服务器命令、工作目录或基础参数还不能正常运行。

也可以在终端直接验证同一个二进制：

```powershell
vide --version
```

Windows 自定义服务器示例：

```powershell
D:/tools/vide/vide.exe --version
```

如果配置了自定义服务器，再继续检查：

- `vide.server.command` 使用绝对路径
- 这个命令能在终端正常执行 `--version`
- `vide.server.args` 和 `vide.server.additionalArgs` 都是字符串数组
- `vide.server.cwd` 如果设置，必须是已经存在的目录
- 修改 `vide.server.command`、`vide.server.args`、`vide.server.additionalArgs`、`vide.server.cwd` 或 `vide.trace.server` 后，需要重启语言服务器

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

### 文件变化没有触发刷新

默认 `vide.files.watcher` 是 `client`，会优先使用 VS Code 的文件变化通知。客户端不支持动态监听文件时，Vide 会回退到服务端监听。

如果工程文件变化后没有触发刷新，可以先临时切到服务端监听：

```json
{
  "vide.files.watcher": "server"
}
```

`vide.files.excludeDirs` 只接受工作区相对目录，不支持 glob。项目文件选择请优先使用 `vide.toml` 的 `sources` 和 `exclude`；如果还要减少 VS Code 自己的文件监听事件，再配置 VS Code 的 `files.watcherExclude`。
