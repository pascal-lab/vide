---
title: 解析与分析模型
description: 说明 Vizsla 如何从文件发现、解析、索引到语义分析和诊断。
---

Vizsla 会把“看见文件”和“把文件当作工程编译单元”分开处理。这样缺少完整配置时仍然可以开箱即用地阅读代码, 而写入清单后又能得到更准确的语义分析。

## 分层模型

Vizsla 的工程分析分成四层:

1. 文件发现和加载: 根据 workspace、`vizsla.toml`、`libraries` 和全局排除项决定哪些文件进入 VFS。
2. 单文件解析: 对 Verilog/SystemVerilog 文件建立语法树, 并产生 parse diagnostics。
3. Best-effort 索引: 对已加载文件建立可用于跳转、引用、悬停等读能力的索引。
4. Semantic profile: 根据显式工程配置生成编译视图, 用于跨文件 semantic diagnostics、include/define 处理、top module 和 Qihe project mode。

每个加载出来的 source root 都会有一个角色:

| 角色 | 用途 |
| --- | --- |
| `Local` | 当前工程的语义 root。通常来自显式 `sources` 或显式 `include_dirs`。 |
| `BestEffortIndex` | 只做 best-effort 索引, 不进入编译 profile。 |
| `Library` | 依赖库 root, 会参与引用工程的 semantic profile。 |
| `Ignored` | 不参与解析、索引或诊断。 |

`BestEffortIndex` 是默认可读性的关键: 它让跳转、引用等功能尽量能用, 但不会把这些文件假装成一个准确的编译工程。

## Manifest 行为

`vizsla.toml` 控制工程模型, 但不是直接控制某一条 diagnostics 入口。不同配置会生成不同 root 和 profile:

| 配置状态 | 文件加载 | Semantic profile |
| --- | --- | --- |
| 没有清单 | 默认索引 workspace root | 不生成 |
| 清单存在但省略 `sources` | 默认索引 workspace root | 不生成 |
| 省略 `sources`, 但写了 `include_dirs` | 默认索引 workspace root, 另加载 include root | include root 生成 profile; 默认索引 root 不进入 profile |
| `sources = []` | 不做默认 workspace 索引 | 不生成 |
| `sources = []` 且写了 `include_dirs` | 只加载 include root | include root 生成 profile |
| `sources = ["rtl/**"]` | 加载匹配的 source root | 生成 profile |

显式 `sources = []` 是关闭 workspace 索引的 opt-out 路径。省略 `sources` 表示启用默认 best-effort 索引, 两者语义不同。

当 `sources` 显式非空且 `include_dirs` 省略时, Vizsla 会把 `sources` 推导出的扫描根目录作为默认 include 目录。显式写 `include_dirs = []` 时不会回退。

## 解析和诊断

Parse diagnostics 来自单文件解析。它不需要完整 semantic profile, 但会受 source root 角色影响:

| Root 角色 | Parse diagnostics 范围 | Semantic diagnostics |
| --- | --- | --- |
| `Local` | workspace root 内文件 | 需要 profile |
| `Library` | workspace root 内文件 | 需要 profile |
| `BestEffortIndex` | 只处理打开文件 | 不运行 |
| `Ignored` | 不运行 | 不运行 |

Semantic diagnostics 一定要通过 profile。没有 profile 的 root 不会被提升成项目编译单元; 默认索引 root 也不会生成项目编译计划。

因此你可能看到多条入口触发诊断刷新, 例如打开文件、保存文件、workspace refresh 或 VS Code 的 Problems 面板请求, 但最终都会落到同一套分层模型上: root 角色决定范围, profile 决定是否能做跨文件语义分析。

## 导航能力

跳转、引用、悬停、补全和 code lens 会尽量使用已加载文件的索引信息。默认索引可以让这些读能力开箱即用, 但它不是准确编译配置:

- 如果 workspace 里有重名模块或多个候选定义, 结果可能需要用户选择。
- 如果 include、macro、library 配置缺失, 部分语义能力会降级或缺失。
- 如果需要和真实工程编译一致, 应该显式配置 `sources`, `include_dirs`, `defines`, `libraries` 和 `top_modules`。

这种设计的目标是先让工程可读, 再通过清单把结果变准。

## Qihe project mode

Qihe 的 project mode 使用 semantic profile 生成的编译计划。只有计划里存在实际编译源文件时, Vizsla 才会传入工程文件、`--top`、`-I` 和 `-D` 参数。

如果当前文件只属于默认 `BestEffortIndex` root, 或者没有可用的项目编译 root, Vizsla 会回到单文件输入。这样默认索引不会误触发项目模式。
