---
title: 当扩展无法正常启动
description: 按状态栏、输出通道和版本命令检查 Vide 扩展启动问题。
---

这页用于确认 Vide 扩展和语言服务器有没有正常接上。命令和日志入口见 [操作参考](../commands-status-logs/)；已经出现高级启动或日志问题时，转到 [高级故障排查](../troubleshooting/)。

## 1. 看状态栏

先看 VS Code 右侧状态栏的 `Vide` 状态项：

| 状态 | 下一步 |
| --- | --- |
| 普通 `Vide` 文本 | 语言服务器已经接上。悬停查看项目配置是否已加载。 |
| 旋转图标长时间不结束 | 继续看 `Vide Language Server` 输出通道。 |
| 警告图标 | 点击状态项确认是否缺少项目配置文件。 |
| 错误图标 | 点击状态项查看菜单顶部错误，然后打开输出通道。 |

点击状态项或执行 `Vide：显示状态` 可以打开状态菜单。

## 2. 打开语言服务器输出

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

## 3. 验证服务器版本

执行 `Vide：显示服务器版本`。扩展会使用当前启动命令、工作目录和环境，把 `vide.server.args` 与 `--version` 组合起来执行版本查询；不会附加 `vide.server.additionalArgs`。如果配置了 `vide.server.command`，也会使用这个自定义命令。

也可以在终端直接验证二进制：

```powershell
vide --version
```

Windows 自定义服务器示例：

```powershell
D:\tools\vide\vide.exe --version
```

## 4. 核对扩展自带服务器或自定义服务器

默认配置使用扩展自带的服务器二进制。扩展会在安装目录的 `server` 子目录查找：

- Windows: `vide.exe`
- macOS/Linux: `vide`

如果配置了 `vide.server.command`，输出通道应出现：

```text
[INFO] Using custom server command: ...
```

建议把自定义服务器写成绝对路径，并先用上一步的 `--version` 单独验证。

## 5. 需要时打开服务器日志文件

如果语言服务器能启动，但需要查看更详细的内部日志，可以通过 `vide.server.additionalArgs` 传入：

```json
{
  "vide.server.additionalArgs": [
    "--log",
    "debug",
    "--log_file",
    "D:\\work\\chip\\.vide\\server.log"
  ]
}
```

保存后选择扩展提示里的 `重启`，或执行 `Vide：重启语言服务器`。如果进程启动前就失败，仍然先看 `Vide Language Server` 输出通道。
