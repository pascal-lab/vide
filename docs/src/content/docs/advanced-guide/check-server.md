---
title: 当扩展无法正常启动
description: 用一条自检流程确认 Vide 扩展能否启动语言服务器。
---

这页只处理“扩展是否把语言服务器拉起来了”这一类问题。命令 ID、状态项含义和输出通道清单见 [命令、状态和日志](../../user-guide/commands-status-logs/)；已经确认进程能启动但行为异常时，转到 [高级故障排查](../troubleshooting/)。

## 1. 先进入状态菜单

点击 VS Code 右侧状态栏的 `Vide`，或执行 `Vide：显示状态`。这一步只用来判断下一步入口：

| 看到的情况 | 下一步 |
| --- | --- |
| 菜单顶部没有错误，悬停信息显示服务器已连接 | 启动路径通常正常；如果只是没有诊断或跳转结果，继续查项目配置。 |
| 菜单顶部提示语言服务器错误 | 记下错误文本，然后打开 `Vide Language Server` 输出通道。 |
| 状态长时间停在启动中 | 直接打开 `Vide Language Server` 输出通道。 |
| 菜单提示缺少项目配置文件 | 启动通常不是问题，先创建或打开项目配置文件。 |

## 2. 看语言服务器输出

执行 `Vide：显示语言服务器输出`，或在状态菜单里选择显示输出。一次正常启动通常能看到：

```text
[INFO] Vide extension activating...
[INFO] Platform: win32-x64
[INFO] Looking for bundled server at: ...
[INFO] Server command: ...
[INFO] Server args: ...
[INFO] Working directory: ...
[INFO] Language server started successfully
```

这些行能确认扩展看到的平台、最终启动命令、传参和工作目录。

如果这些行里没有 `Language server started successfully`，优先看最后一条错误。常见分支是：

- 找不到扩展自带服务器：继续核对安装包或目标平台。
- 自定义命令不存在或无权限：继续验证自定义服务器路径。
- 进程启动后立即退出：先运行版本命令，再看是否需要打开服务器日志。

## 3. 验证服务器命令

执行 `Vide：显示服务器版本`。如果这个命令也失败，说明扩展当前使用的服务器命令、工作目录或基础参数还不能正常运行。

也可以在终端直接验证同一个二进制：

```powershell
vide --version
```

Windows 自定义服务器示例：

```powershell
D:\tools\vide\vide.exe --version
```

## 4. 分清扩展自带服务器和自定义服务器

默认配置使用扩展自带的服务器二进制。扩展会在安装目录的 `server` 子目录查找：

- Windows: `vide.exe`
- macOS/Linux: `vide`

如果配置了 `vide.server.command`，输出通道应出现：

```text
[INFO] Using custom server command: ...
```

建议把自定义服务器写成绝对路径，并先用上一步的 `--version` 单独验证。自定义命令能在终端运行但扩展里失败时，再核对 `vide.server.cwd`、`vide.server.args` 和 PATH 环境差异。

## 5. 进程能启动但仍异常

如果输出通道已经显示服务器启动成功，但功能仍异常，启动链路本身通常已经通过。接下来按 [高级故障排查](../troubleshooting/) 打开更详细的服务器日志，或回到具体功能页检查项目配置、诊断、跳转和 Qihe 设置。
