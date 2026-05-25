---
title: 按症状故障排查
description: 按状态栏、启动、Qihe、诊断、项目扫描和文件监听症状定位问题。
---

这页按症状处理问题。先确认服务器是否能启动时，用 [服务器自检流程](./check-server.md)；查命令、状态项和输出通道入口时，用 [操作参考](./commands-status-logs.md)。

## `Vizsla` 状态栏显示错误或警告

点击 `Vizsla` 状态项打开状态菜单。项目配置错误会显示在菜单顶部；也可以选择显示输出，或执行 `Vizsla：显示语言服务器输出`。

优先看这些输出：

- `Bundled Vizsla Language Server binary not found`
- `Unsupported platform-architecture combination`
- `Failed to start language server`
- `Server command`
- `Server args`
- `Working directory`

如果错误来自项目配置，先打开或修正工作区根目录（你用 VS Code 打开的顶层目录）下的 `vizsla.toml`。

## 找不到扩展自带服务器

扩展默认在自己的安装目录下寻找 `server/vizsla.exe` 或 `server/vizsla`。本地开发时，只运行 `npm run compile` 不会生成这个服务器二进制。

可以在 `editors/vscode` 下打包 debug VSIX：

```powershell
npm run package:debug
```

也可以直接配置本地服务器：

```json
{
  "vizsla.server.command": "D:\\Proj\\vizsla\\target\\release\\vizsla.exe"
}
```

保存后选择提示里的 `重启`，或执行 `Vizsla：重启语言服务器`。

## 自定义启动命令、参数或工作目录启动失败

检查这些点：

- `vizsla.server.command` 使用绝对路径，并能在终端执行 `--version`。
- `vizsla.server.args` 和 `vizsla.server.additionalArgs` 都是字符串数组。
- `vizsla.server.cwd` 如果设置，必须是已存在目录。
- 修改 `vizsla.server.command`、`vizsla.server.args`、`vizsla.server.additionalArgs`、`vizsla.server.cwd` 或 `vizsla.trace.server` 后，接受扩展提示里的 `重启`。

示例：

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.args": [],
  "vizsla.server.cwd": "D:\\work\\chip",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

## Qihe 命令不可用或按钮没有效果

`Vizsla：运行 Qihe 分析` 只对本地 Verilog/SystemVerilog 文件可用。目标必须是 `file:` URI，扩展名必须是 `.v`、`.vh`、`.sv`、`.svh` 或 `.svi`。

如果 Qihe 进程无法启动：

- 确认 `vizsla.qihe.command` 是可执行文件名或绝对路径，不是项目目录。
- 如果使用 `qihe`，确认它在 VS Code 进程看到的 `PATH` 中。
- 在 Windows 上，VS Code 从桌面启动时看到的 `PATH` 可能和终端不同；不确定时先写绝对路径。

## Qihe 分析失败

运行 Qihe 时会出现独立的 `Qihe` 状态项。失败后点击该状态项会打开 `Vizsla Qihe` 输出通道；错误通知里的 `显示 Qihe 输出` 也会打开同一通道。

在 `Vizsla Qihe` 中检查目标文件、Qihe compile/run 参数、Qihe 输出和最后的失败信息。Qihe 参数默认会从 `vizsla.toml` 推导；已经由脚本管理参数的工程，可以在 [VS Code 设置](./vscode-settings.md#qihe) 中关闭自动推导并显式配置 compile/run 参数。

## 诊断太频繁或不更新

默认 `vizsla.diagnostics.update` 是 `onSave`，保存时刷新诊断。大型工程建议保持这个默认值。

如果需要编辑时刷新：

```json
{
  "vizsla.diagnostics.update": "onType"
}
```

如果诊断不更新，先保存文件，再执行 `Vizsla：重新加载项目配置`。如果仍然不更新，执行 `Vizsla：重启语言服务器` 并查看项目配置错误。

## 实例端口或参数报错

如果 `Problems` 面板提示实例连接或参数有问题，先把光标放到对应实例附近，打开灯泡菜单。Vizsla 目前支持这些场景：

- 端口没有接全：使用 `补全连接`。
- 参数没有值：使用 `补全参数`。
- 端口混用了有序写法和命名写法：使用 `将有序端口连接转换为命名连接`，或用 `移除空端口连接` 清理多余空连接。
- 参数混用了有序写法和命名写法：使用 `将有序参数赋值转换为命名赋值`。
- 写了 `.port` 但缺少 `()`：使用 `添加显式空端口连接`。
- 实例完全没有端口列表：使用 `添加空实例端口列表`。

如果灯泡里没有这些操作，先确认目标模块能被 Vizsla 找到。最直接的检查方式是在实例模块名上执行 `Go to Definition`：跳不过去时，优先修正 `sources`、`include_dirs`、`defines` 或 `libraries`。Vizsla 当前不会自动修复找不到模块、找不到 include 或 import 解析失败这类问题。

## 想隐藏或降低某类诊断

把光标放在对应诊断位置，打开灯泡菜单。对于带有可识别诊断代码的 slang 诊断，Vizsla 会提供写入用户设置或工作区设置的快速修复，例如忽略此类诊断，或把错误降级为 warning。

如果灯泡里没有这些选项，可以手动编辑 `vizsla.diagnostics.slang.rules`。规则写法见 [VS Code 设置](./vscode-settings.md#diagnostics)。

## 项目文件没有被扫描

检查项目配置文件：

- `vizsla.toml` 是否位于工作区根目录。旧版 `vizsla_config.toml` 仍可使用，但两个文件同时存在时优先读取 `vizsla.toml`。
- 如果写了 `sources`，路径模式是否能匹配目标文件。例如 `rtl/*.sv` 只匹配 `rtl` 目录下一层的 `.sv` 文件；递归目录要写成 `rtl/**`。
- 显式 `sources = []` 会关闭工作区索引。
- `exclude` 路径模式是否把目标文件排除了，例如目录递归排除是 `build/**`。
- 文件扩展名是否是 `.v`、`.sv`、`.vh`、`.svh`、`.svi` 或 `.map`。
- 是否打开了子目录，导致工作区根目录变了。

这里的路径模式是带 `*` 和 `**` 的 glob 写法；`*` 不跨目录，`**` 可以跨目录。`sources` 和 `exclude` 里不要用 Windows 反斜杠 `\`，统一写 `/`。

目录末尾加不加 `/` 要分场景看：`include_dirs = ["include"]` 和 `include_dirs = ["include/"]` 都是在写 include 搜索目录，文档里统一推荐不带 `/`；但 `sources = ["rtl/"]` 不是“递归扫描 `rtl` 下所有文件”的写法，想扫目录请写 `sources = ["rtl/**"]`。

扩展创建的默认 `vizsla.toml` 会写入 `sources = []`；需要索引项目时，请写入实际 `sources` 路径模式，并按需补充 `include_dirs`、`defines`、`libraries` 或 `top_modules`。手写配置省略 `sources` 时，Vizsla 会尽力扫描工作区，方便基础跳转和阅读，但不会启用完整的跨文件诊断视图。

## include 或宏没有生效

把 include 目录和宏写入项目配置文件：

```toml
defines = ["SYNTHESIS", "WIDTH=32"]
include_dirs = ["include", "rtl"]
```

如果显式写了 `include_dirs = []`，Vizsla 不会回退到 `sources`。

## 格式化没有结果或失败

默认格式化后端调用 `verible-verilog-format`。如果本机没有这个命令，配置：

```json
{
  "vizsla.formatter.path": "D:\\tools\\verible\\verible-verilog-format.exe"
}
```

格式化失败通常会显示外部格式化工具输出的错误。先减少自定义 `vizsla.formatter.args`，用默认参数验证。

## 文件变化没有触发刷新

默认 `vizsla.files.watcher` 是 `client`，会优先使用 VS Code watched-file notifications。客户端不支持动态 watched files 时会回退到服务端 watcher。

如果工程文件变化后没有触发刷新：

```json
{
  "vizsla.files.watcher": "server"
}
```

`vizsla.files.excludeDirs` 只接受工作区相对目录，不支持 glob。文件选择请优先使用项目配置文件的 `sources` / `exclude` 路径模式；如果还要减少 VS Code 自己的 watcher 事件，另配 VS Code 的 `files.watcherExclude`。

## 需要更详细的服务器日志

如果进程能启动，但需要服务器内部日志，在 `vizsla.server.additionalArgs` 中添加 `--log` 和 `--log_file`，然后重启语言服务器。具体步骤见 [服务器自检流程](./check-server.md)。
