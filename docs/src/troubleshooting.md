# 故障排查

遇到问题时，请按顺序检查。不要一上来就改很多设置，这样反而更难判断是哪一步出了问题。

## 状态栏显示 Vizsla Error

1. 运行 `Vizsla: Show Language Server Output`。
2. 找到第一条 `[ERROR]`。
3. 看错误是否属于下面几类。

### Bundled server binary not found

> [!NOTE]
> **含义**
>
> 扩展没有找到随扩展包发布的 `vizsla` 或 `vizsla.exe`。

1. 如果你是从扩展市场安装的，请确认扩展安装完整，并尝试重新安装。
2. 如果你从源码运行扩展，请先打包或手动编译服务器。
3. 临时解决时，可以设置：

```json
{
  "vizsla.server.command": "D:/Proj/vizsla/target/debug/vizsla.exe"
}
```

然后运行 `Vizsla: Restart Language Server`。

### Unsupported platform-architecture combination

> [!NOTE]
> **含义**
>
> 当前系统和 CPU 架构不在扩展支持列表里。

当前支持：

- `darwin-arm64`
- `darwin-x64`
- `linux-arm64`
- `linux-x64`
- `win32-arm64`
- `win32-x64`

如果你的平台不在列表中，请从源码编译服务器，并配置 `vizsla.server.command`。

### Failed to query Vizsla server version

> [!NOTE]
> **含义**
>
> 扩展尝试运行 `vizsla --version` 失败。

1. 打开输出窗口，确认 `Server command`。
2. 在终端里手动运行同一个命令。
3. 如果命令不可执行，检查文件路径和执行权限。
4. macOS/Linux 上如果文件没有执行权限，运行：

```bash
chmod +x /path/to/vizsla
```

## 状态栏一直停在 Starting

1. 打开输出窗口。
2. 检查服务器命令是否存在。
3. 检查 `vizsla.server.cwd` 是否指向真实目录。
4. 如果你设置了 `--log_file`，检查日志目录是否可写。

> [!NOTE]
> **讨论**
>
> 语言服务器通过 stdio 和 VS Code 通信。不要把 `vizsla.server.command` 设置成会启动交互 shell、等待用户输入、或者输出大量非 LSP 内容的脚本。

## 没有诊断

先确认：

1. 文件后缀是 `.v`、`.vh`、`.sv`、`.svh` 或 `.svi`。
2. VS Code 右下角语言模式是 Verilog 或 SystemVerilog。
3. 状态栏是 `Vizsla Ready`。
4. `vizsla.diagnostics.enable` 是 `true`。
5. `vizsla.diagnostics.parse.enable` 或 `vizsla.diagnostics.semantic.enable` 没有被关掉。

默认 `vizsla.diagnostics.update` 是 `onSave`。请先保存文件，再看 Problems 面板。

## include 文件找不到

表现

你在代码里写了：

```systemverilog
`include "defs.svh"
```

但 Vizsla 诊断说找不到 include 或宏未定义。

在 `vizsla_config.toml` 里加入 include 目录：

```toml
sources = ["rtl"]
include_dirs = ["include", "rtl"]
```

保存配置后，再保存当前 `.sv` 文件或重启服务器。

## 宏没有生效

在 `vizsla_config.toml` 里写 `defines`：

```toml
defines = ["SYNTHESIS", "WIDTH=32"]
```

如果宏来自某个头文件，请优先配置 `include_dirs`，并确认相关文件真的被 include。

## 第三方库里的模块解析不到

把第三方库路径放到 `libraries`：

```toml
sources = ["rtl"]
libraries = ["ip/vendor"]
```

如果库里有不希望分析的目录，用 `exclude` 排掉：

```toml
exclude = ["ip/vendor/examples", "ip/vendor/doc"]
```

## 保存配置后没有变化

1. 确认配置文件名是 `vizsla_config.toml`。
2. 确认它在 VS Code 打开的工作区根目录。
3. 确认 `vizsla.workspace.auto.reload` 是 `true`。
4. 运行 `Vizsla: Restart Language Server`。
5. 打开输出窗口确认工程信息被重新加载。

## 补全不弹出

1. 确认文件语言模式正确。
2. 输入触发字符，例如 `.`, `(`, `,`, `@`, `#`, 或反引号 `` ` ``。
3. 手动按 VS Code 的 trigger suggest 快捷键。
4. 检查工程是否有严重语法错误。

> [!NOTE]
> **讨论**
>
> 补全依赖当前文件能被解析。未闭合的括号、缺失的 `endmodule`、错误的宏分支都可能影响补全质量。

## 重命名漏改

1. 先修复 include、宏、库路径配置。
2. 保存所有相关文件。
3. 重新执行重命名。

> [!WARNING]
> **警告**
>
> 跨文件重命名依赖 Vizsla 能看到完整工程。配置不完整时，重命名会更保守。

## 格式化结果不符合预期

1. 检查 `vizsla.formatting.indent.width`。
2. 如果配置了外部 formatter，检查 `vizsla.formatter.path` 是否存在。
3. 检查 `vizsla.formatter.args` 是否与 formatter 支持的参数一致。
4. 临时清空 `vizsla.formatter.path`，确认默认格式化是否正常。

## 文件太多导致卡顿

1. 在 `vizsla_config.toml` 里缩小 `sources`。
2. 用 `exclude` 排除构建输出、仿真输出、生成文件。
3. 在 VS Code 设置中加入：

```json
{
  "vizsla.files.excludeDirs": ["build", "target", "sim/out"],
  "files.watcherExclude": {
    "**/build/**": true,
    "**/target/**": true,
    "**/sim/out/**": true
  }
}
```

4. 大工程保持 `vizsla.diagnostics.update` 为 `onSave`。

