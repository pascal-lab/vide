# 命令、状态栏和日志

## 命令面板命令

VS Code 扩展贡献了三个命令:

| 命令 | 作用 |
| --- | --- |
| `Vizsla: Show Language Server Output` | 打开 `Vizsla Language Server` 输出通道。 |
| `Vizsla: Restart Language Server` | 停止并重新启动语言服务器。 |
| `Vizsla: Show Server Version` | 执行服务器 `--version`, 并显示第一行版本输出。 |

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
