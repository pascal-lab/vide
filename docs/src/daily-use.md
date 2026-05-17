# 日常使用

这一章按 IDE 工作流介绍 Vizsla 的能力。我们尽量把每个功能写成“你能看到什么、什么时候会触发、哪些配置会影响它”, 方便你在 VS Code 里逐项验证。

## 语言识别和语法高亮

VS Code 扩展会注册两类语言:

| 语言 | 文件扩展名 |
| --- | --- |
| Verilog | `.v`, `.vh` |
| SystemVerilog | `.sv`, `.svh`, `.svi` |

打开这些文件后, VS Code 会应用 Vizsla 提供的 TextMate grammar 和语言配置。语法高亮本身不依赖语言服务器启动成功; 即使服务器还在启动, 你也应该先看到基础高亮、注释规则和括号匹配。

语言服务器启动后, 我们会继续提供语义层能力, 例如诊断、跳转、补全、语义高亮、inlay hints 和 code lens。

## 诊断

Vizsla 的诊断分成两层:

- Parse diagnostics: 解析阶段发现的语法问题。
- Semantic diagnostics: 编译/语义阶段发现的问题, 例如实例、端口、参数、类型和引用相关错误。

默认配置是:

- `vizsla.diagnostics.enable`: `true`
- `vizsla.diagnostics.parse.enable`: `true`
- `vizsla.diagnostics.semantic.enable`: `true`
- `vizsla.diagnostics.update`: `onSave`

默认在保存时刷新诊断。大型 RTL 工程里我们建议保留 `onSave`, 这样不会在每次输入时触发较重的语义刷新。如果你希望边写边看问题, 可以改成:

```json
{
  "vizsla.diagnostics.update": "onType"
}
```

诊断会出现在 VS Code 的 `Problems` 面板和编辑器下划线里。部分 quick fix 会读取诊断携带的稳定数据, 因此你通常需要先看到相关诊断, 才会看到对应代码操作。

slang warning 相关配置放在 `vizsla.diagnostics.slang.*` 下。warning 名称、warning group 和 `-W...` 语义请看 [VS Code 设置](./vscode-settings.md#diagnostics) 中的 Diagnostics 说明。

## 跳转到定义和声明

我们同时提供 `Go to Definition` 和 `Go to Declaration`。在日常 RTL 阅读里, 你可以用它们处理这些场景:

- 从实例化位置跳到目标模块定义。
- 从信号引用跳到声明位置。
- 从端口、参数、typedef、函数、任务等名字跳到对应定义或声明。
- 在同文件和跨文件引用之间导航, 前提是这些文件被当前工程加载。

`Go to Declaration` 找不到更合适声明时, 会回退到 definition 逻辑。VS Code 支持 location link 时, 我们会返回更完整的源范围和目标范围; 否则返回普通 location。

如果跳转结果不符合预期, 先检查工程是否加载到了目标文件。没有 `vizsla_config.toml` 时, Vizsla 只把当前 workspace root 当作未配置工程; 更复杂目录建议显式配置 `sources`, `include_dirs` 和 `libraries`。

## 查找引用和文档高亮

`Find References` 会基于语义解析结果查找符号引用, 比纯文本搜索更适合 RTL 代码阅读。文档高亮则是局部版本: 光标停在符号上时, VS Code 可以高亮当前文件里相关的引用位置。

这两个功能会受到 `vizsla.scope.visibility` 影响:

| 设置 | 行为 |
| --- | --- |
| `private` | 默认值。scope 内部符号默认不暴露到其它 scope, 端口除外。 |
| `public` | 放宽 scope 可见性, 引用和重命名会搜索更宽范围。 |

如果你发现局部变量、generate block 或命名块里的符号引用范围过大或过小, 可以优先确认这个设置。

## 重命名

Vizsla 支持 `Prepare Rename` 和 `Rename Symbol`。VS Code 在真正重命名前会先询问服务器当前位置是否允许重命名, 这样可以避免在关键字、字面量或不稳定位置上误触发。

重命名会基于引用搜索生成 workspace edit, 因此它和 `Find References` 一样依赖工程语义信息。我们建议在执行重命名前先确认:

- 目标文件都已经被工程加载。
- 当前符号能正确跳转或查找引用。
- `scope.visibility` 符合你的工程习惯。

## 补全

补全会在常见 Verilog/SystemVerilog 输入点触发。当前服务器声明的触发字符包括:

```text
. ( , @ # ` newline
```

我们根据当前位置选择不同补全来源:

- 预处理指令: 在反引号位置补 `define`, `include`, `ifdef`, `ifndef`, `elsif`, `pragma`, `timescale`, `default_nettype` 等。
- 关键字和片段: 在 module item、过程语句、generate、specify、config、library map 等上下文里给出合适关键字和 snippets。
- 表达式候选: 在赋值右侧、条件表达式、过程语句、函数/任务参数等位置补当前可见值。
- 成员访问: 在 `.` 后补结构成员、层次成员或可解析成员名。
- 端口和参数: 在命名连接 `.port(...)`、命名参数 `#(.PARAM(...))` 和有序连接位置补候选。
- 敏感列表: 在 `@` 或事件控制上下文中补信号和事件关键字。
- 系统任务/函数: 在表达式或语句上下文中补 `$display`、`$bits` 等 slang 提供的系统子程序事实。

补全会尽量避免在注释、字符串和字面量中弹出无关候选。对于实例端口/参数相关补全, 工程里必须能解析到被实例化的目标模块。

## Snippets

Vizsla 内置了一组 Verilog/SystemVerilog snippets。它们和普通关键字补全一起出现, 但只有 VS Code 客户端声明支持 snippet 时才会返回 snippet edit。

常见片段包括:

- 顶层声明: `module`, `primitive`, `macromodule`, `config`。
- library map: `library`, `include`。
- 参数列表: `parameter`, `localparam`。
- module item: `wire`, `reg`, `genvar`, `generate`, `function`, `task`, `assign`, `always`, `initial`。
- 控制语句: `if`, `ifelse`, `case`, `casez`, `casex`, `for`, `while`, `repeat`, `forever`, `wait`。
- 预处理指令: `define`, `include`, `ifdef`, `ifndef`, `elsif`, `pragma`, `timescale`, `default_nettype`。

这些 snippets 不是简单全局列表, 我们会按语法上下文过滤。例如 `module` 更适合顶层, `parameter` 会在参数端口列表或 module item 中出现, 过程语句片段不会随意出现在顶层。

## 悬停

悬停会优先识别名字和字面量:

- 对符号名, 我们会解析定义并渲染模块、端口、参数、声明、实例、函数等信息。
- 对端口连接 shorthand, 我们会同时展示 port 和 local 两侧的信息。
- 对字面量, 我们会渲染解析后的 literal 信息。

VS Code 支持 Markdown hover 时, 我们会返回 Markdown; 否则返回纯文本。悬停信息依赖当前位置是否能解析到语义定义, 如果工程没有加载完整, 信息会相应减少。

## 签名帮助

签名帮助会在 `(`, `,`, `.` 触发, 主要服务两个场景:

- 模块实例端口连接: 展示目标模块端口列表, 并根据当前位置标记当前 active parameter。
- 参数赋值列表: 展示目标模块参数列表。

默认情况下, 签名中会尽量带上端口或参数的类型/声明信息。如果你只想看到参数相关内容, 可以打开:

```json
{
  "vizsla.signature.help.params.only": true
}
```

签名帮助同样依赖实例目标能被解析。目标模块缺失、工程未加载依赖库或 include/define 配置不完整时, 签名帮助可能不会出现。

## 格式化

Vizsla 支持三类格式化入口:

- `Format Document`
- `Format Selection`
- 按 Enter 时的 on-type formatting

默认 formatter provider 是 `verible`, 会调用外部 `verible-verilog-format`。如果它不在 `PATH` 中, 请配置:

```json
{
  "vizsla.formatter.path": "D:\\tools\\verible\\verible-verilog-format.exe"
}
```

`vizsla.formatter.args` 会传给 `verible-verilog-format`。服务器还会根据编辑器传入的 `tabSize` 为 verible 追加当前缩进宽度对应的 `--indentation_spaces=<N>`。

按 Enter 时的辅助格式化不只是调用 formatter。我们还会处理注释续行和上一行结构格式化, 受这些设置控制:

- `vizsla.formatting.on.enter`
- `vizsla.formatting.in.comments`
- `vizsla.formatting.indent.width`

如果你只想关闭 Enter 时的行为, 不影响手动 `Format Document`, 可以只关 `vizsla.formatting.on.enter`。

## 代码操作

当前代码操作围绕模块实例、端口连接和参数赋值修复。修复类操作通常在相关诊断出现后作为 quick fix 展示, 转换类操作也可作为 refactor 展示。

| 操作 | 用途 |
| --- | --- |
| `Fill connections` | 补齐缺失端口连接。命名连接会补 `.name()`, 有序连接会尝试补可用同名信号或占位表达式。 |
| `Fill parameters` | 补齐缺失参数赋值。命名参数会补 `.PARAM(...)`, 有序参数会按目标参数顺序补值。 |
| `Convert ordered port connections to named connections` | 把有序端口连接改写成命名端口连接。 |
| `Convert ordered parameter assignments to named assignments` | 把有序参数赋值改写成命名参数赋值。 |
| `Remove empty port connections` | 删除命名端口列表里多余的空连接, 例如末尾多出的逗号。 |
| `Add explicit empty port connection` | 给隐式空端口连接补出显式空括号。 |
| `Add empty instance port list` | 给缺失端口列表的实例补 `()`。 |

这些操作的目标是减少机械 RTL 编辑。我们会基于解析出的目标模块端口/参数顺序生成 edit, 因此它们需要实例目标可解析。

## 语义高亮

Vizsla 提供 semantic tokens。相比 TextMate 高亮, semantic tokens 来自语义分析, 可以区分更多 RTL 角色。

当前端口高亮有两类可配置增强:

- `vizsla.semantic.tokens.port.clk.rst.enable`: 对 1-bit clock/reset 风格端口打专用标记。clock 名称会匹配 `clock`, `clk`, `tck`; reset 名称会匹配常见 `reset` / `rst` 形式。
- `vizsla.semantic.tokens.port.input.output.enable`: 根据端口方向打 read/write/ref modifier。`input` 映射 read, `output` 映射 write, `inout` 映射 read + write, `ref` 映射 ref。

如果主题支持对应 semantic token 类型和 modifier, 你会看到 clock/reset、输入输出端口和普通符号之间更清晰的颜色差异。

## 折叠和大纲

我们支持 folding ranges 和 document symbols。

折叠范围会覆盖常见结构, 包括模块、块、语句、注释等。VS Code 会把这些范围显示为可折叠区域, 帮助你收起长模块、generate 块、case 语句或大段注释。

文档符号会填充 VS Code Outline 视图。我们会从 HIR/source map 中收集模块、config、UDP、library、端口、参数、net/data declaration、typedef、实例、block、function、generate、specify 等符号。客户端支持 hierarchical document symbols 时, Outline 会保留层级结构; 不支持时, 我们会返回扁平 symbol information。

## 选择范围

Vizsla 支持 selection range。你可以用 VS Code 的扩大/缩小选择命令, 从当前 token 逐步扩展到表达式、语句、块或更大的语法结构。

这个功能适合重构前选中一段 RTL 结构, 也适合在复杂表达式中快速选中当前子表达式。

## Inlay Hints

默认启用三类 inlay hints:

| 设置 | 默认值 | 说明 |
| --- | --- | --- |
| `vizsla.inlayHints.port.connection.enable` | `true` | 为有序端口连接或空连接显示目标端口名。 |
| `vizsla.inlayHints.parameter.assignment.enable` | `true` | 为有序参数赋值显示目标参数名。 |
| `vizsla.inlayHints.end.structure.enable` | `true` | 在模块结构结尾显示结构名。 |

端口和参数 hints 主要解决 RTL 实例化可读性问题。例如有序连接 `u(a, b, c)` 需要对照目标模块端口表才能理解, hint 会直接显示 `clk:`, `rst_n:`, `data:` 这类标签。

对于有序连接和参数赋值, hint 还会携带目标位置和可选 text edit。支持这些能力的客户端可以把 hint 当作导航或快速转换入口。

## 实例 Code Lens

默认启用模块实例 code lens:

```json
{
  "vizsla.lens.instantiations.enable": true
}
```

我们会在模块声明处显示实例数量, 解析后的标题形如 `0 instances`, `1 instance` 或 `N instances`。这个数量来自全工程引用搜索, 用来帮助你判断某个模块是否被实例化、实例化规模有多大。

当前 code lens 只显示数量, 没有绑定跳转命令。如果你需要定位具体实例, 请使用 `Find References`。
