# 确认 Vizsla 正常工作

装好扩展、打开工程之后，我们先确认语言服务器真的启动了。

## 看状态栏

看 VS Code 左下角或状态栏区域。Vizsla 会显示这些状态之一：

- `Vizsla Starting`：服务器正在启动。
- `Vizsla Ready`：服务器已经运行。
- `Vizsla Stopped`：服务器已经停止。
- `Vizsla Error`：服务器启动失败或运行失败。

看到 `Vizsla Ready` 就可以继续写代码。

## 查看服务器版本

打开命令面板，运行：

```text
Vizsla: Show Server Version
```

如果 VS Code 弹出类似 `Vizsla server: vizsla 0.1.0_DEBUG` 或 `vizsla 0.1.0_RELEASE` 的信息，说明扩展已经能启动服务器二进制。

## 打开输出日志

如果状态不是 `Ready`，先打开输出窗口：

```text
Vizsla: Show Language Server Output
```

你也可以直接点击状态栏里的 Vizsla 状态项。

输出里会有这些信息：

- 扩展路径。
- 当前平台，例如 `win32-x64`。
- VS Code 版本。
- 服务器命令。
- 工作目录。
- 服务器启动或失败的原因。

## 重启服务器

当你修改了服务器启动设置，或者觉得服务器状态不对时，运行：

```text
Vizsla: Restart Language Server
```

重启后状态栏会从 `Vizsla Stopping`、`Vizsla Starting` 回到 `Vizsla Ready`。

> [!WARNING]
> **警告**
>
> 如果你修改的是普通语言功能设置，比如诊断、格式化、inlay hints，VS Code 会通过配置更新通知服务器。只有这些启动相关设置变化后，扩展才会提示你重启：
>
> - `vizsla.server.command`
> - `vizsla.server.args`
> - `vizsla.server.additionalArgs`
> - `vizsla.server.cwd`
> - `vizsla.trace.server`

