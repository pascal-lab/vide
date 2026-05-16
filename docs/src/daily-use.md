# 日常使用指南

这一页按你每天写代码时会遇到的动作来讲。你不需要一次记住所有功能，遇到什么需求就看哪一节。

## 看诊断

Vizsla 会给 Verilog/SystemVerilog 文件提供诊断。诊断分两类：

- 解析诊断：语法写错、括号不完整、语句结构不合法等。
- 语义诊断：模块、端口、参数、include、宏、连接关系等更深层的问题。

打开一个 `.sv` 文件，故意写一个错误：

```systemverilog
module top(input logic clk
endmodule
```

保存文件后，VS Code 的 Problems 面板应该出现来自 Vizsla 或 slang 的诊断。

> [!NOTE]
> **讨论**
>
> 默认诊断刷新策略是 `onSave`。也就是说你保存文件后才刷新诊断。这样做是为了避免大工程在每次输入时都重新跑昂贵的诊断。如果你希望边写边刷新，可以把 `vizsla.diagnostics.update` 改成 `onType`。

## 自动补全

Vizsla 会在常见触发字符后提供补全：

- `.`
- `(`
- `,`
- `@`
- `#`
- 反引号：`` ` ``

在实例化端口里输入 `.`，等待补全列表：

```systemverilog
child u_child(
    .
);
```

VS Code 应该弹出可连接端口名。选择补全项后，Vizsla 会插入对应文本。如果 VS Code 支持 snippet，Vizsla 会优先给出更适合继续填写的 snippet。

## 跳转到定义

把光标放在信号、模块名、实例名、generate 块名、task/function、library 名等符号上，然后按 `F12`，或者右键选择 `Go to Definition`。

VS Code 会跳到这个符号定义的位置。

## 跳转到声明

右键符号，选择 `Go to Declaration`。

> [!NOTE]
> **讨论**
>
> 如果 Vizsla 找不到单独的声明位置，它会回退到定义位置。这样你不会因为声明/定义模型不同而空手回来。

## 查找引用

把光标放在符号上，按 `Shift+F12`，或者右键选择 `Find All References`。

VS Code 会列出该符号的定义和引用位置。

## 重命名

把光标放在要改名的符号上，按 `F2`，输入新名字。

Vizsla 会在它能确认的范围内一起修改引用。比如信号声明、端口连接、task 调用、generate 块引用等。

> [!WARNING]
> **警告**
>
> 重命名依赖工程解析结果。如果 include 目录、宏或库路径配置不完整，Vizsla 可能无法看到所有引用。重命名前请先确认 Problems 面板里没有明显的工程配置类错误。

## 鼠标悬停

把鼠标放在符号上。

VS Code 会显示 Vizsla 提供的 hover 内容。支持 Markdown 的客户端会看到更友好的格式；不支持时会退回纯文本。

## 文档符号和大纲

打开 VS Code 的 Outline 面板，或者运行 `Go to Symbol in Editor`。

你可以看到模块、端口、参数、变量、实例、generate 块、task/function、library 等符号结构。

## 工作区符号

运行 `Go to Symbol in Workspace`，输入模块名或符号名。

VS Code 会在当前工作区里搜索符号。

## 折叠代码

Vizsla 会为模块、块、语句结构等提供 folding ranges。

点击编辑器左侧的折叠箭头。

对应代码块会折叠。

## 选择范围

使用 VS Code 的 `Expand Selection` 或 `Shrink Selection`。

选择范围会按语法结构逐步扩大或缩小，而不是只按单词或行处理。

## 格式化

Vizsla 支持三种格式化入口：

- Format Document
- Format Selection
- 按 Enter 时的 on-type formatting

打开命令面板，运行：

```text
Format Document
```

当前文件会按配置的缩进宽度格式化。默认缩进宽度是 4 个空格。

> [!NOTE]
> **讨论**
>
> 如果你配置了外部 formatter，Vizsla 会使用 `vizsla.formatter.path` 和 `vizsla.formatter.args`。如果没有配置，则使用默认格式化路径。

## inlay hints

Vizsla 默认开启三类 inlay hints：

- 端口连接提示。
- 参数赋值提示。
- 结构结束名提示。

打开包含模块实例化或复杂结构的 `.sv` 文件。

编辑器会在合适的位置显示灰色的辅助提示。你可以在 VS Code 设置里分别关闭它们：

- `vizsla.inlayHints.port.connection.enable`
- `vizsla.inlayHints.parameter.assignment.enable`
- `vizsla.inlayHints.end.structure.enable`

## code lens

Vizsla 默认开启模块实例化相关 code lens。

打开包含模块定义或实例化的文件。

相关位置上方会显示可点击的 code lens。你可以用 `vizsla.lens.instantiations.enable` 控制是否启用。

## 语义高亮

Vizsla 会提供比普通 TextMate 语法高亮更细的语义 token。比如端口、时钟复位端口、输入输出端口可以被额外标记。

确保 VS Code 开启了 semantic highlighting。然后打开包含端口声明的文件。

主题如果支持相关 token modifier，你会看到端口类符号拥有更稳定的高亮。

## signature help

在参数列表或端口列表里输入 `(`、`,` 或 `.`。

VS Code 会弹出 signature help，帮助你确认当前正在填写哪个参数或端口。

如果你只想显示参数相关 signature help，可以打开：

```json
{
  "vizsla.signature.help.params.only": true
}
```

## code action

Vizsla 会在一些诊断上提供快速修复。

常见修复包括：

- Fill connections：补齐缺失端口连接。
- Fill parameters：补齐缺失参数。
- Convert ordered ports：把有序端口连接转换成命名端口连接。
- Convert ordered params：把有序参数赋值转换成命名参数赋值。
- Add implicit named port parens：把 `.a` 修成 `.a()`。
- Add instance parens：给缺少括号的实例补上 `()`。

当 VS Code 在代码旁边显示小灯泡时，点击它，选择 Vizsla 提供的修复。

选择修复后，相关代码会被编辑器自动改写。

> [!WARNING]
> **警告**
>
> 这些修复依赖诊断信息。没有对应诊断时，Vizsla 不会盲目给出修复项。

