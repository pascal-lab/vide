---
title: 索引、项目配置和诊断
description: 解释 Vide 在无配置和有配置工程中如何加载文件、建立项目分析、产生诊断并调用 Qihe。
---

这页解释 Vide 的分析边界：为什么没有 `vide.toml` 也能做一些跳转和补全，为什么写好 `vide.toml` 后诊断、重命名和 Qihe 会变得更完整。

字段怎么写见 [配置第一个项目](../../user-guide/first-project/) 和 [项目配置参考](../../user-guide/project-configuration/)。本页只解释这些字段会改变什么。

## 三种工作状态

Vide 会根据工作区里是否存在 `vide.toml`，以及 `sources` 是否显式配置，进入不同的工作状态：

| 状态 | 会加载什么 | 适合什么 |
| --- | --- | --- |
| 没有 `vide.toml` | 扫描工作区里的 Verilog/SystemVerilog 文件，作为尽力索引 | 先读代码、做基础跳转、引用、悬停和补全 |
| 有 `vide.toml`，但省略 `sources` | 继续做尽力索引；如果写了 `include_dirs`，这些目录会作为 include 搜索目录加载 | 过渡状态，不建议长期依赖 |
| `sources = []` | 明确不扫描工作区源码；如果写了 `include_dirs`，只加载这些 include 目录 | 刚创建模板、暂时不希望 Vide 猜测工程结构 |
| `sources = ["rtl/**"]` | 按 `sources` 加载工程源码，并结合 `include_dirs`、`defines`、`libraries`、`top_modules` 建立项目分析 | 正式工程配置 |

可以把它记成三句话：

- 省略 `sources`：先帮我读一下工作区，但不要把扫描结果当成正式工程。
- `sources = []`：不要自动扫描源码。
- `sources = ["rtl/**"]`：这些文件就是当前工程源码。

## 尽力索引和项目分析

尽力索引用于阅读代码。它会尽量加载工作区里的 RTL 文件，让跳转、引用、悬停、补全、实例数量提示等功能尽早可用。它不等于真实编译配置，也不会启用完整的工程级语义诊断和工程重命名。

项目分析来自 `vide.toml`。当 `sources` 指向真实源码后，Vide 会把这些文件放进项目视图，并把 `include_dirs`、`defines`、`libraries` 和 `top_modules` 用于跨文件解析、诊断、重命名和 Qihe 工程分析。

`libraries` 会作为依赖工作区加载，参与当前工程分析。`exclude` 用来从已经加载的文件中排除生成文件、仿真输出或黑盒文件。路径和 glob 写法见 [项目配置参考](../../user-guide/project-configuration/#sources-和-exclude-的路径和-glob)。

## include 目录怎么参与分析

`include_dirs` 只表示 include 搜索路径，不表示这些目录里的文件都是独立编译入口。

头文件（`.vh`、`.svh`、`.svi`）通常通过被 `.v` 或 `.sv` 文件 `` `include `` 后参与解析。只打开头文件，或者只把目录写进 `include_dirs`，不等于 Vide 会为这个头文件单独跑完整工程诊断。

如果显式写了 `sources` 但省略 `include_dirs`，Vide 会从 `sources` 推导默认 include 目录。例如 `sources = ["rtl/**/*.sv"]` 会把 `rtl` 作为默认 include 目录。显式写成 `include_dirs = []` 会关闭这个回退。

## 诊断为什么会不一样

诊断分两层：

- 单文件解析诊断：只需要看当前文件，例如语法错误、括号不匹配、解析失败。
- 跨文件语义诊断：需要工程信息，例如目标模块、include 目录、宏分支、库文件和 top module。

常见行为如下：

| 文件状态 | 解析诊断 | 跨文件语义诊断 |
| --- | --- | --- |
| 被 `sources` 明确加载的源文件 | 可运行 | 可运行 |
| 通过 `include_dirs` 找到的头文件 | 通过 include 它的源文件参与解析 | 通过 include 它的源文件参与项目分析 |
| 只来自尽力索引的文件 | 通常只处理打开文件 | 不作为工程诊断入口 |
| 被 `exclude` 过滤的文件 | 不运行 | 不运行 |

因此，无配置工程里看到基础诊断是正常的；需要跨文件语义诊断时，应先写好 `vide.toml`。

## 跳转和同名模块

跳转、引用、悬停、补全和实例数量提示会优先使用已经加载的索引信息。尽力索引能让这些功能在无配置工程中先工作，但它只能做编辑器阅读层面的推断。

在项目分析里，同名 module 按当前工程视图处理。Vide 不会把目录名当成隐式命名空间；如果多个同名 module 同时可见，工程本身需要通过项目配置、库边界或编译脚本消除歧义。

在尽力索引里，如果一个实例能对应多个同名 module，Vide 只为阅读功能做一次就近推测：优先同文件，再优先共同目录最深的候选，最后优先同一扫描根。只有出现唯一最优候选时才采用；如果打平，结果保持歧义。

这种推测不是 SystemVerilog 语言规则。能唯一就近选择时不会报告诊断；无法唯一选择时，会用 `ambiguous-module-instantiation` 这类提示级诊断标出。配置好的工程仍按更严格的语义规则处理；如果开启了第三方 SystemVerilog 编译前端 `slang` 的语义诊断，Vide 会优先展示它给出的诊断。

## Qihe 什么时候按工程运行

Qihe 集成复用 Vide 的项目配置发现结果。工作目录下存在可用 `vide.toml`，并且编译计划里有实际源文件时，Vide 会传入工程文件、`--top`、`-I` 和 `-D` 参数。

如果当前文件只来自尽力索引，或者项目配置没有形成可用编译计划，Vide 会让 Qihe 回到单文件输入。这样可以避免把无配置扫描结果误当成正式工程配置。

Qihe 的命令形状、参数配置和失败排查见 [Qihe 分析](../../user-guide/features/qihe/)。
