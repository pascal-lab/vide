---
title: VS Code 设置
description: Vizsla VS Code 扩展的配置项参考。
---

所有设置都在 `vizsla.*` 命名空间下。你可以在 VS Code Settings UI 中搜索 `Vizsla`, 也可以直接编辑 `settings.json`。

## Server

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.server.command` | `null` | 自定义语言服务器命令。留空时使用 bundled server。 |
| `vizsla.server.args` | `[]` | 启动服务器时传入的前置参数。 |
| `vizsla.server.additionalArgs` | `[]` | 启动服务器时追加的参数。 |
| `vizsla.server.cwd` | `null` | 服务器工作目录。默认使用第一个 workspace folder, 没有 workspace 时使用扩展目录。 |
| `vizsla.trace.server` | `"off"` | LSP 通信跟踪, 可选 `"off"`, `"messages"`, `"verbose"`。 |

示例:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

## Files

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.files.excludeDirs` | `[]` | workspace 相对目录排除列表。不支持 glob。 |
| `vizsla.files.watcher` | `"client"` | 文件监听方式, 可选 `"client"`, `"notify"`, `"server"`。 |

`client` 会优先使用 VS Code 的 watched-file notifications。当前服务器配置中, 客户端不支持动态 watched files 时会回退到 server-side watcher; `notify` 和 `server` 都会走服务端监听路径。

## Workspace

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.workspace.auto.reload` | `true` | 工程清单变更后自动刷新工程信息。 |

## Tombi

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.tombi.schemaInjection.enable` | `true` | 检测到 Tombi VS Code 扩展已安装时, 自动把 Vizsla schema 关联写入 Tombi 配置。 |
| `vizsla.tombi.schemaInjection.scope` | `"user"` | schema 关联写入范围。可选 `"user"`, `"workspace"`。 |

默认的 `"user"` 会写入 Tombi 用户级配置, 避免污染工程目录。`"workspace"` 会写入每个 file workspace folder 下的 Tombi 配置; 如果已有 `.tombi.toml` 则写它, 否则写 `tombi.toml`。未安装 Tombi 扩展时, Vizsla 不会写入配置也不会弹出提示。

## Scope

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.scope.visibility` | `"private"` | 控制 scope 内符号对其它 scope 的可见性。可选 `"private"`, `"public"`。 |

这个设置会影响 references, rename 和 document highlight。

## Formatter 和 Formatting

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.formatter.provider` | `"verible"` | formatter 后端。当前支持 `verible`, 会调用外部 `verible-verilog-format`。 |
| `vizsla.formatter.path` | `null` | `verible` provider 使用的可执行文件路径。留空时查找 `verible-verilog-format`。 |
| `vizsla.formatter.args` | `["--failsafe_success=false"]` | 传给 `verible-verilog-format` 的参数。 |
| `vizsla.formatting.on.enter` | `true` | 按 Enter 时启用格式化行为。 |
| `vizsla.formatting.in.comments` | `true` | 在注释内启用 Enter 辅助格式化。 |
| `vizsla.formatting.indent.width` | `4` | 编辑器没有提供 formatting options 时使用的后备缩进宽度。 |

`Format Document`, `Format Selection` 和 on-type formatting 请求会优先使用编辑器传入的 `tabSize`。`verible` provider 会在 formatter args 后追加当前缩进宽度对应的 `--indentation_spaces=<N>`。

## Inlay Hints

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.inlayHints.port.connection.enable` | `true` | 显示端口连接 inlay hints。 |
| `vizsla.inlayHints.parameter.assignment.enable` | `true` | 显示参数赋值 inlay hints。 |
| `vizsla.inlayHints.end.structure.enable` | `true` | 显示结构结束名 hints。 |

## Lens

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.lens.instantiations.enable` | `true` | 显示模块实例 code lens。 |

## Semantic Tokens

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.semantic.tokens.port.clk.rst.enable` | `true` | 为 clock/reset 端口启用专用 semantic token modifier。 |
| `vizsla.semantic.tokens.port.input.output.enable` | `true` | 为 input/output 端口启用专用 semantic token modifier。 |

## Diagnostics

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.diagnostics.enable` | `true` | 启用所有 Vizsla diagnostics。 |
| `vizsla.diagnostics.update` | `"onSave"` | 诊断刷新时机。可选 `"onSave"`, `"onType"`。 |
| `vizsla.diagnostics.parse.enable` | `true` | 启用语法和 parse diagnostics。 |
| `vizsla.diagnostics.semantic.enable` | `true` | 启用编译和 semantic diagnostics。 |
| `vizsla.diagnostics.slang.warnings` | `[]` | slang warning 选项, 例如 `default`, `everything`, `none`, `error`, `no-<name>`, `error=<name>`。 |
| `vizsla.diagnostics.slang.rules` | `[]` | 诊断过滤或 severity override 规则。 |

`vizsla.diagnostics.slang.warnings` 会传给 slang 的 parse/semantic diagnostics 接口。写法和 slang 的 `-W...` warning options 对齐, 但在 VS Code 设置里不写前导 `-W`: 例如 `everything` 对应 `-Weverything`, `no-unused` 对应 `-Wno-unused`, `error=width-trunc` 对应 `-Werror=width-trunc`。

需要查 warning 名称、warning group 或 warning flag 语义时, 请优先看 slang 文档:

- [slang Warning Reference](https://sv-lang.com/warning-ref.html): 完整 warning 名称和分组。
- [slang Command Line Reference](https://sv-lang.com/command-line-ref.html): `-Wfoo`, `-Wno-foo`, `-Wnone`, `-Weverything`, `-Werror` 等 warning option 的行为。
- [slang User Manual](https://sv-lang.com/user-manual.html): `pragma diagnostic` 和 `slang lint_off` / `lint_on` 这类源码内诊断控制方式。

`vizsla.diagnostics.slang.rules` 的 selector 支持:

- `code:<subsystem>:<code>`
- `option:<name>`
- `group:<name>`
- `source:parse`
- `source:semantic`

示例:

```json
{
  "vizsla.diagnostics.slang.rules": [
    { "selector": "source:parse", "severity": "warning" },
    { "selector": "option:unconnected-port", "severity": "ignore" }
  ]
}
```

`severity` 可选 `ignore`, `info`, `warning`, `error`, `fatal`。

## Signature Help

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.signature.help.params.only` | `false` | 只显示参数相关签名帮助。 |
