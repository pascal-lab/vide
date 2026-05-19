# 故障排查

## 状态栏显示 Vizsla Error

先点击状态栏, 或执行 `Vizsla: Show Language Server Output`。重点看:

- `Bundled Vizsla Language Server binary not found`
- `Unsupported platform-architecture combination`
- `Failed to start language server`
- `Server command`
- `Server args`
- `Working directory`

如果 bundled server 缺失, 换对应平台 VSIX, 或配置 `vizsla.server.command` 指向本地服务器。

## 找不到 bundled server

扩展默认在自己的安装目录下寻找 `server/vizsla.exe` 或 `server/vizsla`。本地开发时, 如果你只运行了 `npm run compile`, 通常还没有 bundled server。你可以:

```powershell
cd editors\vscode
npm run package
```

或者直接配置本地服务器:

```json
{
  "vizsla.server.command": "D:\\Proj\\vizsla\\target\\release\\vizsla.exe"
}
```

## 自定义 command/args/cwd 启动失败

检查这些点:

- `vizsla.server.command` 建议使用绝对路径。
- `vizsla.server.args` 必须是字符串数组。
- `vizsla.server.additionalArgs` 必须是字符串数组。
- `vizsla.server.cwd` 如果设置, 必须指向存在的目录。
- 修改启动参数后要重启语言服务器。

示例:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.args": [],
  "vizsla.server.cwd": "D:\\work\\chip",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

## 诊断太频繁或不更新

默认 `vizsla.diagnostics.update` 是 `onSave`, 保存时刷新诊断。大型工程建议保持这个默认值。

如果你希望编辑时刷新:

```json
{
  "vizsla.diagnostics.update": "onType"
}
```

如果诊断不更新, 先保存文件。然后执行 `Vizsla: Restart Language Server`, 并查看输出通道里是否有工程加载错误。

## 项目文件没有被扫描

检查工程清单:

- `vizsla_config.toml` 是否位于 workspace root。
- `sources` 是否包含目标目录。
- `exclude` 是否把目录排除了。
- 文件扩展名是否是 `.v`, `.sv`, `.vh`, `.svh`, `.svi` 或 `.map`。
- 你是否打开了子目录, 导致 workspace root 变了。

VS Code 扩展会在缺少清单时创建默认 `vizsla_config.toml`。如果你使用其它客户端且没有清单, Vizsla 只保留 syntax/parse diagnostics; 需要语义诊断和跨文件能力时请先创建清单。我们不会自动向父目录或子目录搜索清单。

## include 或宏没有生效

把 include 目录和宏写入清单:

```toml
defines = ["SYNTHESIS", "WIDTH=32"]
include_dirs = ["include", "rtl"]
```

如果你显式写了 `include_dirs = []`, 我们不会回退到 `sources`。

## 格式化没有结果或失败

默认 formatter provider 会调用 `verible-verilog-format`。如果本机没有这个命令, 配置:

```json
{
  "vizsla.formatter.path": "D:\\tools\\verible\\verible-verilog-format.exe"
}
```

格式化失败时, 输出通常来自 formatter stderr。你也可以减少自定义 `vizsla.formatter.args`, 先用默认参数验证。

## 文件监听问题

默认 `vizsla.files.watcher` 是 `client`, 我们会优先使用 VS Code watched-file notifications。客户端不支持动态 watched files 时会回退到服务端 watcher。

如果工程文件变化后没有触发刷新:

```json
{
  "vizsla.files.watcher": "server"
}
```

`vizsla.files.excludeDirs` 只接受 workspace 相对目录, 不支持 glob。需要 glob 时, 另配 VS Code 的 `files.watcherExclude`。

## 日志排查

把服务器日志写到文件:

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

然后执行 `Vizsla: Restart Language Server`。如果服务器没有启动到读取参数阶段, 仍然先看 VS Code 的 `Vizsla Language Server` 输出通道。
