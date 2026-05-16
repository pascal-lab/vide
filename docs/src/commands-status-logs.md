# 命令、状态栏和日志

VS Code 扩展启动后，会注册三个命令，并在状态栏显示服务器状态。遇到问题时，这一页是第一张检查清单。

## 命令

### `Vizsla: Show Language Server Output`

打开 `Vizsla Language Server` 输出窗口。

当服务器没有启动、诊断不刷新、配置不生效时，先运行这个命令。

### `Vizsla: Restart Language Server`

停止并重新启动语言服务器。

修改服务器路径、服务器参数、工作目录或 trace 设置后，运行这个命令。

### `Vizsla: Show Server Version`

运行服务器的 `--version`，并把第一行结果显示出来。

如果你不确定 VS Code 正在使用哪个 `vizsla`，运行这个命令，然后打开输出窗口看完整命令。

## 状态栏

Vizsla 状态栏项会显示：

- `Vizsla Starting`
- `Vizsla Ready`
- `Vizsla Stopping`
- `Vizsla Stopped`
- `Vizsla Error`

点击状态栏项可以直接打开输出窗口。

## 日志里应该看什么

打开输出窗口后，优先看这些行：

- `Platform`：确认平台是不是你预期的系统和架构。
- `Looking for bundled server at`：确认扩展在找哪个服务器文件。
- `Server command`：确认最终运行的命令。
- `Server args`：确认传入参数。
- `Working directory`：确认工程根目录。
- `Language server started successfully`：确认启动成功。

## 服务器命令行参数

Vizsla 服务器本身支持这些命令行参数：

```text
--process-name <PROCESS_NAME>
--log <LOG>
--log_file <LOG_FILE>
--version
--help
```

常用例子：

```powershell
vizsla --version
vizsla --log debug --log_file D:\tmp\vizsla.log
```

在 VS Code 里传参时，请放到 `vizsla.server.additionalArgs`：

```json
{
  "vizsla.server.additionalArgs": [
    "--log",
    "debug",
    "--log_file",
    "D:/tmp/vizsla.log"
  ]
}
```

> [!WARNING]
> **警告**
>
> `--log_file` 的路径目录必须存在，或者 Vizsla 能创建它。路径写错时，服务器可能启动失败。启动失败后请看输出窗口里的错误信息。

