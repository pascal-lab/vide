---
title: 解析与分析模型
description: 解释 Vizsla 为什么有时只能读文件，有时可以做完整项目分析。
---

这页适合在你遇到下面问题时阅读：

- 为什么没有 `vizsla.toml` 时，跳转还能工作，但跨文件诊断不一定完整？
- 为什么写了 `sources = []` 后，Vizsla 不再自动扫描工作区？
- 为什么头文件只放进 `include_dirs`，不一定会单独出现在诊断里？
- 为什么 Qihe 有时按工程分析，有时退回单文件分析？

先解释三个词：

- 尽力索引：Vizsla 在没有完整项目配置时，尽量读取工作区里的 Verilog/SystemVerilog 文件，让跳转、引用、悬停、补全等阅读功能先可用。
- 项目分析：Vizsla 根据 `vizsla.toml` 里的 `sources`、`include_dirs`、`defines`、`libraries` 和 `top_modules` 建立的工程视图。跨文件诊断和 Qihe 工程分析依赖它。
- 诊断：VS Code `Problems` 面板里的错误、警告和提示。单文件语法问题可以不依赖完整项目配置；跨文件语义问题需要项目分析。

## sources 是最重要的开关

`sources` 决定 Vizsla 是否把某些文件当成“这个工程的一部分”。

| 配置状态 | Vizsla 会读什么 | 项目分析 |
| --- | --- | --- |
| 没有项目配置文件 | 尽力索引工作区 | 不建立 |
| 有项目配置文件，但省略 `sources` | 尽力索引工作区 | 不为这些默认索引文件建立 |
| 省略 `sources`，但写了 `include_dirs` | 尽力索引工作区，并加载 include 目录 | include 目录可被项目分析使用；默认索引文件不参与 |
| `sources = []` | 不自动扫描工作区 | 不建立 |
| `sources = []` 且写了 `include_dirs` | 只加载 include 目录 | include 目录可被项目分析使用 |
| `sources = ["rtl/**"]` | 加载匹配的源文件 | 建立 |

可以把它记成一句话：省略 `sources` 是“先帮我读一下工作区，但不要假装它已经配置好了”；`sources = []` 是“不要自动扫工作区”；`sources = ["rtl/**"]` 是“这些文件属于我的工程”。

`include_dirs` 只负责 include 搜索。如果你显式写了 `sources`，但没有写 `include_dirs`，Vizsla 会从 `sources` 推导一个默认 include 目录。例如 `sources = ["rtl/**/*.sv"]` 会把 `rtl` 作为默认 include 目录。省略 `sources` 时不会做这个推导；显式写成 `include_dirs = []` 时也不会使用回退。

`libraries` 会作为依赖工作区加载，并参与当前工程的项目分析。`exclude` 是工作区相对 glob，用来从已加载文件中过滤掉生成文件、仿真目录或黑盒文件。glob 的写法见 [项目配置](./project-configuration.md#路径和-glob-怎么写)。

## 诊断为什么会不一样

单文件解析诊断只需要看当前文件，例如少了分号、括号不配对。跨文件语义诊断需要知道更多工程信息，例如 include 目录、宏定义、库路径和哪些文件属于同一个工程。

常见结果如下：

| 文件所在状态 | 单文件解析诊断 | 跨文件语义诊断 |
| --- | --- | --- |
| 被 `sources` 明确加载的 `.v` 或 `.sv` 文件 | 可以运行 | 可以运行，前提是项目分析可用 |
| 通过 `include_dirs` 找到的头文件 | 通过 include 它的源文件参与解析 | 通过 include 它的源文件参与项目分析 |
| 只在尽力索引里出现的文件 | 通常只处理打开文件 | 不运行 |
| 被 `exclude` 过滤的文件 | 不运行 | 不运行 |

头文件（`.vh`、`.svh`、`.svi`）通常不是独立编译入口。它们主要通过被 `.v` 或 `.sv` 文件 include 后参与解析和诊断。只打开头文件，或只把目录写进 `include_dirs`，不等于会为这个头文件单独跑完整诊断。

## 跳转和同名模块

跳转、引用、悬停、补全和实例数量提示会优先使用已经加载的索引信息。尽力索引能让这些功能尽早可用，但它不是严格的编译配置。

在项目分析里，同名 module 会按工程视图处理。如果多个同名 module 同时可见，Vizsla 不会把目录名当成隐式命名空间。

在尽力索引里，如果多个同名 module 都可见，Vizsla 会只为编辑器阅读功能做一次就近推测：优先同文件，再优先共同目录最深的候选，最后优先同一扫描根。只有出现唯一最优候选时才会采用；如果打平，结果仍然保持 ambiguous。

这种推测不是 SystemVerilog 语言规则。能唯一就近选择时不会报告诊断；无法唯一选择时，会用 `ambiguous-module-instantiation` 这类提示级诊断标出。配置好的工程里，同名 module 的问题仍然按更严格的语义规则处理；如果你开启了第三方 SystemVerilog 编译前端 `slang` 的语义诊断，Vizsla 会优先展示它给出的诊断。

## Qihe 工程分析

当前 Qihe 自动工程分析要求工作目录下存在 `vizsla.toml`。只有旧版 `vizsla_config.toml` 时，普通 VS Code 功能仍会兼容读取它，但 Qihe 会退回单文件输入。

存在 `vizsla.toml` 时，Qihe 会使用项目分析得到的编译计划。只有计划里存在实际源文件时，Vizsla 才会传入工程文件、`--top`、`-I` 和 `-D` 参数。

如果当前文件只来自尽力索引，或者没有可用的项目编译计划，Vizsla 会让 Qihe 回到单文件输入。这样可以避免默认索引误触发工程分析。
