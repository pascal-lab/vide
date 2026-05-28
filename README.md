<p align="center">
  <a href="https://vide.pascal-lab.net/">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="docs/src/assets/vide-logo-reveal-dark.svg">
      <img src="docs/src/assets/vide-logo-reveal-light.svg" alt="Vide" width="520">
    </picture>
  </a>
</p>

# Vide - 现代 SystemVerilog 开发环境

[![Homepage](https://img.shields.io/badge/homepage-vide.pascal--lab.net-0969da)](https://vide.pascal-lab.net/)
[![CI](https://github.com/pascal-lab/vide/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/pascal-lab/vide/actions/workflows/ci.yml)
[![Release](https://img.shields.io/badge/release-v0.1.6-2ea44f)](https://vide.pascal-lab.net/changelog/v0-1-6/)
[![VS Code Marketplace](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fpascal-lab.gallery.vsassets.io%2F_apis%2Fpublic%2Fgallery%2Fpublisher%2Fpascal-lab%2Fextension%2Fvide-ls%2Flatest%2Fassetbyname%2FMicrosoft.VisualStudio.Code.Manifest&query=%24.version&label=Marketplace&prefix=v&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTIzLjE1IDIuNTkgMTguMjEuMjFhMS40OSAxLjQ5IDAgMCAwLTEuNy4yOUw3LjA2IDkuMTEgMi45NCA1Ljk5YTEgMSAwIDAgMC0xLjI3LjA2TC4zMyA3LjI2YTEgMSAwIDAgMCAwIDEuNDhMMy45IDEyIC4zMyAxNS4yNmExIDEgMCAwIDAgMCAxLjQ4bDEuMzQgMS4yMWExIDEgMCAwIDAgMS4yNy4wNmw0LjEyLTMuMTIgOS40NSA4LjYxYTEuNDkgMS40OSAwIDAgMCAxLjcuMjlsNC45NC0yLjM4QTEuNSAxLjUgMCAwIDAgMjQgMjAuMDZWMy45NGExLjUgMS41IDAgMCAwLS44NS0xLjM1Wk0xOCAxNy40NSAxMC44MyAxMiAxOCA2LjU1djEwLjlaIi8%2BPC9zdmc%2B&logoColor=white&color=007ACC)](https://marketplace.visualstudio.com/items?itemName=pascal-lab.vide-ls)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

Vide 是专为 Verilog/SystemVerilog 开发者打造的现代化开发环境，旨在让硬件设计像软件开发一样流畅顺手。Vide 提供了[十多项](https://vide.pascal-lab.net/user-guide/features/)在现代软件开发环境中已成标配、却长期缺失于硬件开发环境的能力，包括但不限于[定义跳转](https://vide.pascal-lab.net/user-guide/features/navigation/)、[代码注解](https://vide.pascal-lab.net/user-guide/features/inlay-hints/)、[精准补全](https://vide.pascal-lab.net/user-guide/features/completion/)和[自动重构](https://vide.pascal-lab.net/user-guide/features/quick-fixes/)等。借助 Vide，硬件开发者可以更高效地理解、编写和维护 Verilog/SystemVerilog 代码。

## 功能展示

### 符号导航

在 Vide 中使用[定义跳转](https://vide.pascal-lab.net/user-guide/features/navigation/)、[引用搜索](https://vide.pascal-lab.net/user-guide/features/references/)和[符号大纲](https://vide.pascal-lab.net/user-guide/features/document-symbols/)在模块、端口和寄存器之间快速定位，让开发者不用离开当前上下文也能追清 RTL 连接关系。

| Peek Definition | Find All References | Document Symbol |
| --- | --- | --- |
| <img src="docs/src/assets/homepage-features/peek-definition.png" alt="Peek Definition 截图" width="360" /> | <img src="docs/src/assets/homepage-features/find-all-references.jpeg" alt="Find All References 截图" width="360" /> | <img src="docs/src/assets/homepage-features/document-symbol.jpeg" alt="Document Symbol 截图" width="360" /> |

### 代码理解

利用 Vide 的[悬停信息](https://vide.pascal-lab.net/user-guide/features/hover/)和[代码注解](https://vide.pascal-lab.net/user-guide/features/inlay-hints/)在一个窗口中实时查看模块、字面量与端口连接信息，减少窗口切换的负担，让开发者更专注于 RTL 设计本身。

|  |  |
| --- | --- |
| <img src="docs/src/assets/homepage-features/hover-on-module-name.png" alt="模块 Hover 信息截图" width="520" /><br />Module Hover | <img src="docs/src/assets/homepage-features/hover-on-instance-name.png" alt="例化 Hover 信息截图" width="520" /><br />Instance Hover |
| <img src="docs/src/assets/homepage-features/hover-on-number-literal.png" alt="字面量 Hover 信息截图" width="520" /><br />Number Literal Hover | <img src="docs/src/assets/homepage-features/inlay-hints.png" alt="Inlay Hints 截图" width="520" /><br />Inlay Hints |

### 精准补全

Vide 的[补全](https://vide.pascal-lab.net/user-guide/features/completion/)机制理解当前代码上下文，能在实例化、端口连接和其他编辑位置给出更贴近工程语义的建议，也能通过代码片段提供结构化补全。

|  |  |  |
| --- | --- | --- |
| <img src="docs/src/assets/homepage-features/completion-module-decl.png" alt="模块声明补全截图" width="360" /><br />Module Declaration | <img src="docs/src/assets/homepage-features/completion-ports.png" alt="端口补全截图" width="360" /><br />Port Completion | <img src="docs/src/assets/homepage-features/completion-items.png" alt="补全候选列表截图" width="360" /><br />Completion Items |
| <img src="docs/src/assets/homepage-features/completion-snippets-module.png" alt="模块代码片段补全截图" width="360" /><br />Module Snippet | <img src="docs/src/assets/homepage-features/completion-module-snippets-expanded.png" alt="展开后的模块代码片段补全截图" width="360" /><br />Expanded Snippet |  |

### 自动重构

通过[自动重构](https://vide.pascal-lab.net/user-guide/features/quick-fixes/)和[重命名](https://vide.pascal-lab.net/user-guide/features/rename/)，把端口连线、信号重命名、转换进制这些繁琐的细节交给 Vide 完成，解放开发者的重构体验。

| Missing Ports | Rename |
| --- | --- |
| <img src="docs/src/assets/homepage-features/missing-ports.png" alt="补全缺失端口 Code Action 截图" width="520" /> | <img src="docs/src/assets/homepage-features/rename-updated.png" alt="重命名符号截图" width="520" /> |

### 诊断分析

Vide 能在编辑过程中实时给出[代码诊断](https://vide.pascal-lab.net/user-guide/features/diagnostics/)，让错误更早被发现。此外，Vide 能够结合[骑河（Qihe）](https://vide.pascal-lab.net/user-guide/features/qihe/)提供的静态分析能力，在编辑器中给出更深入的分析结果，帮助开发者发现潜在问题。

| Undeclared Identifier | Loop Analysis |
| --- | --- |
| <img src="docs/src/assets/homepage-features/diagnostics-undeclared-identifiers.jpeg" alt="未定义标识符诊断截图" width="520" /> | <img src="docs/src/assets/homepage-features/diagnostics-loop-analysis.jpeg" alt="组合环路诊断截图" width="520" /> |

## 继续了解 Vide

- [访问官网](https://vide.pascal-lab.net/)：查看完整功能展示、对比信息和文档入口。
- [在线体验](https://vide.pascal-lab.net/playground/)：直接在浏览器中试用 Vide。
- [阅读用户手册](https://vide.pascal-lab.net/user-guide/)：从快速开始、项目配置和功能特性继续了解。

## 许可证

Vide 使用 [MIT License](LICENSE)。
