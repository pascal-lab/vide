# VS Code 设置参考

这一页是完整设置表。你平时不需要背下来，遇到问题时回来查就行。

在 VS Code 中打开 Settings，搜索 `Vizsla`，可以看到这些设置。也可以直接编辑 `settings.json`：

```json
{
  "vizsla.diagnostics.update": "onSave",
  "vizsla.formatting.indent.width": 4
}
```

## 服务器启动

### `vizsla.trace.server`

控制 VS Code 和语言服务器之间的 LSP 通信日志。

- 默认值：`off`
- 可选值：`off`、`messages`、`verbose`

排查 LSP 通信问题时，可以临时改成 `messages` 或 `verbose`。

### `vizsla.server.command`

自定义语言服务器命令。

- 默认值：`null`

留空时，扩展使用随扩展包发布的 `server/vizsla` 或 `server/vizsla.exe`。从源码调试时，可以设置成自己编译出的路径：

```json
{
  "vizsla.server.command": "D:/Proj/vizsla/target/debug/vizsla.exe"
}
```

### `vizsla.server.args`

传给自定义服务器命令的前置参数。

- 默认值：`[]`

### `vizsla.server.cwd`

服务器进程工作目录。

- 默认值：`null`

留空时，扩展使用第一个 VS Code 工作区文件夹。如果没有工作区，则使用扩展安装目录。

### `vizsla.server.additionalArgs`

追加到服务器命令后的参数。

- 默认值：`[]`

例如把服务器日志级别调高，并写到文件：

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

## 文件和工程

### `vizsla.files.excludeDirs`

工作区相对路径列表。Vizsla 会忽略这些目录。

- 默认值：`[]`

注意这里不支持 glob。如果需要 glob，请同时使用 VS Code 的 `files.watcherExclude`。

```json
{
  "vizsla.files.excludeDirs": ["build", "sim/out", "target"]
}
```

### `vizsla.files.watcher`

控制文件监听方式。

- 默认值：`client`
- 可选值：`client`、`notify`、`server`

含义：

- `client`：优先使用 VS Code 提供的 watched-file 通知。
- `notify`：使用服务器侧 notify watcher。
- `server`：使用服务器侧 watcher。

### `vizsla.workspace.auto.reload`

当工程配置文件变化时，是否自动刷新工程信息。

- 默认值：`true`

## 作用域

### `vizsla.scope.visibility`

控制 scope 内部符号是否对外可见。端口不受这个限制。

- 默认值：`private`
- 可选值：`private`、`public`

如果你希望查找引用、文档高亮、重命名更保守，保持默认 `private`。如果你的代码风格依赖更宽松的可见性，可以尝试 `public`。

## 格式化

### `vizsla.formatter.path`

外部 formatter 可执行文件路径。

- 默认值：`null`

留空时使用默认 formatter。

### `vizsla.formatter.args`

传给 formatter 的参数。

- 默认值：

```json
[
  "--indentation_spaces=4",
  "--failsafe_success=false"
]
```

### `vizsla.formatting.on.enter`

是否在按 Enter 时启用格式化行为。

- 默认值：`true`

### `vizsla.formatting.in.comments`

是否在注释中启用格式化行为。

- 默认值：`true`

### `vizsla.formatting.indent.width`

格式化缩进空格数。

- 默认值：`4`

## inlay hints

### `vizsla.inlayHints.port.connection.enable`

显示端口连接提示。

- 默认值：`true`

### `vizsla.inlayHints.parameter.assignment.enable`

显示参数赋值提示。

- 默认值：`true`

### `vizsla.inlayHints.end.structure.enable`

显示结构结束名提示。

- 默认值：`true`

## code lens

### `vizsla.lens.instantiations.enable`

显示模块实例化相关 code lens。

- 默认值：`true`

## 语义高亮

### `vizsla.semantic.tokens.port.clk.rst.enable`

为时钟和复位端口提供专用语义 token modifier。

- 默认值：`true`

### `vizsla.semantic.tokens.port.input.output.enable`

为输入和输出端口提供专用语义 token modifier。

- 默认值：`true`

## 诊断

### `vizsla.diagnostics.enable`

是否启用所有 Vizsla 诊断。

- 默认值：`true`

### `vizsla.diagnostics.update`

诊断刷新时机。

- 默认值：`onSave`
- 可选值：`onSave`、`onType`

`onSave` 更适合大工程。`onType` 更适合小工程或调试。

### `vizsla.diagnostics.parse.enable`

是否启用语法和解析诊断。

- 默认值：`true`

### `vizsla.diagnostics.semantic.enable`

是否启用编译和语义诊断。

- 默认值：`true`

### `vizsla.diagnostics.slang.warnings`

slang warning 选项列表。写法与 slang 命令行 warning 名称保持一致，例如：

```json
{
  "vizsla.diagnostics.slang.warnings": [
    "default",
    "no-unused",
    "error=unconnected-port"
  ]
}
```

### `vizsla.diagnostics.slang.rules`

按 selector 覆盖诊断严重程度。

支持的 selector：

- `code:<subsystem>:<code>`
- `option:<name>`
- `group:<name>`
- `source:parse`
- `source:semantic`

支持的 severity：

- `ignore`
- `info`
- `warning`
- `error`
- `fatal`

例子：

```json
{
  "vizsla.diagnostics.slang.rules": [
    {
      "selector": "source:parse",
      "severity": "error"
    },
    {
      "selector": "option:unconnected-port",
      "severity": "warning"
    },
    {
      "selector": "code:2:260",
      "severity": "ignore"
    }
  ]
}
```

`force` 字段目前保留给未来支持覆盖源码内诊断 pragma：

```json
{
  "selector": "source:semantic",
  "severity": "warning",
  "force": false
}
```

## signature help

### `vizsla.signature.help.params.only`

是否只显示参数相关 signature help。

- 默认值：`false`
