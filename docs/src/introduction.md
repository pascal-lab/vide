# 欢迎使用 Vizsla

Vizsla 是我们为 Verilog 和 SystemVerilog 准备的语言服务器。本手册会带你快速上手 Vizsla 的使用。

## Vizsla 的能力

Vizsla 目前提供这些能力：

- 语法高亮和语言配置。
- 语法诊断、语义诊断，以及工作区级诊断。
- 代码补全，包括关键字、表达式上下文、端口连接、参数赋值、宏触发等常见位置。
- 鼠标悬停说明。
- 跳转到定义、跳转到声明、类型定义。
- 查找引用、文档高亮、重命名。
- 文档符号、工作区符号、折叠范围、选择范围。
- 全文格式化、范围格式化、按回车键触发的格式化。
- 语义高亮，包括端口、时钟复位端口、输入输出端口等。
- inlay hints，包括端口连接、参数赋值和结构结束名提示。
- code lens，用来查看模块实例化相关信息。
- signature help，用来辅助查看参数和端口列表。
- code action，用来修复缺失端口、缺失参数、混用有序和命名连接、缺少实例括号等问题。

## 从哪开始？

如果你只是使用 Vizsla，请从 [快速上手](./quick-start.md) 或 [安装](./installation.md) 开始。

如果你已经装好了扩展，但项目里 include、宏或库文件识别不对，请直接看 [项目配置](./project-configuration.md)。

如果你是项目开发者，想改 Vizsla 或打包 VS Code 扩展，请看 [从源码构建](./build-from-source.md) 和 [开发者说明](./developer-notes.md)。
