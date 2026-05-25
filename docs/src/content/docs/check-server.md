---
title: 服务器自检流程
description: 按步骤确认 Vizsla 扩展自带服务器或自定义服务器是否正常启动。
---

这页用于确认后台语言服务器能不能正常启动。命令和日志入口见 [操作参考](./commands-status-logs.md)；已经出现具体错误时，转到 [故障排查](./troubleshooting.md)。

## 1. 看状态栏

先看 VS Code 右侧状态栏的 `Vizsla` 状态项：

| 状态 | 下一步 |
| --- | --- |
| 普通 `Vizsla` 文本 | 服务器已经启动。悬停查看项目配置是否已加载。 |
| spinner 长时间不结束 | 继续看 `Vizsla Language Server` 输出通道。 |
| warning 图标 | 点击状态项确认是否缺少项目配置文件。 |
| error 图标 | 点击状态项查看菜单顶部错误，然后打开输出通道。 |

点击状态项或执行 `Vizsla：显示状态` 可以打开状态菜单。

## 2. 打开语言服务器输出

执行 `Vizsla：显示语言服务器输出`，或在状态菜单里选择显示输出。一次正常启动通常能看到：

```text
[INFO] Vizsla extension activating...
[INFO] Platform: win32-x64
[INFO] Looking for bundled server at: ...
[INFO] Server command: ...
[INFO] Server args: ...
[INFO] Working directory: ...
[INFO] Language server started successfully
```

这些行能确认扩展看到的平台、最终服务器命令、传参和工作目录。

## 3. 验证服务器版本

执行 `Vizsla：显示服务器版本`。扩展会使用当前服务器命令、工作目录和环境，将 `vizsla.server.args` 与 `--version` 组合执行版本查询；不会附加 `vizsla.server.additionalArgs`。如果配置了 `vizsla.server.command`，也会使用这个自定义命令。

也可以在终端直接验证二进制：

```powershell
vizsla --version
```

Windows 自定义服务器示例：

```powershell
D:\tools\vizsla\vizsla.exe --version
```

## 4. 核对扩展自带服务器或自定义服务器

默认配置使用扩展自带服务器。扩展会在安装目录的 `server` 子目录查找：

- Windows: `vizsla.exe`
- macOS/Linux: `vizsla`

如果配置了 `vizsla.server.command`，输出通道应出现：

```text
[INFO] Using custom server command: ...
```

建议把自定义服务器写成绝对路径，并先用上一步的 `--version` 单独验证。

## 5. 需要时打开服务器日志文件

如果进程能启动，但需要查看服务器内部日志，可以通过 `vizsla.server.additionalArgs` 传入：

```json
{
  "vizsla.server.additionalArgs": [
    "--log",
    "debug",
    "--log_file",
    "D:\\work\\chip\\.vizsla\\server.log"
  ]
}
```

保存后选择扩展提示里的 `重启`，或执行 `Vizsla：重启语言服务器`。如果进程启动前就失败，仍然先看 `Vizsla Language Server` 输出通道。
