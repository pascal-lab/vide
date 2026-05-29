<p align="center">
  <a href="https://vide.pascal-lab.net/">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/pascal-lab/vide/master/docs/src/assets/vide-logo-dark.png">
      <img src="https://raw.githubusercontent.com/pascal-lab/vide/master/docs/src/assets/vide-logo-light.png" alt="Vide" width="520">
    </picture>
  </a>
</p>

# Vide - 现代 SystemVerilog 编程 IDE

[![Homepage](https://img.shields.io/badge/homepage-vide.pascal--lab.net-0969da)](https://vide.pascal-lab.net/)
[![Playground](https://img.shields.io/badge/playground-try%20online-7c3aed)](https://vide.pascal-lab.net/playground/)
[![English README](https://img.shields.io/badge/README-English-0969da)](README.md)
[![CI](https://github.com/pascal-lab/vide/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/pascal-lab/vide/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/pascal-lab/vide?sort=semver&color=2ea44f)](https://github.com/pascal-lab/vide/releases)
[![VS Code Marketplace](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fpascal-lab.gallery.vsassets.io%2F_apis%2Fpublic%2Fgallery%2Fpublisher%2Fpascal-lab%2Fextension%2Fvide-ide%2Flatest%2Fassetbyname%2FMicrosoft.VisualStudio.Code.Manifest&query=%24.version&label=Marketplace&prefix=v&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTIzLjE1IDIuNTkgMTguMjEuMjFhMS40OSAxLjQ5IDAgMCAwLTEuNy4yOUw3LjA2IDkuMTEgMi45NCA1Ljk5YTEgMSAwIDAgMC0xLjI3LjA2TC4zMyA3LjI2YTEgMSAwIDAgMCAwIDEuNDhMMy45IDEyIC4zMyAxNS4yNmExIDEgMCAwIDAgMCAxLjQ4bDEuMzQgMS4yMWExIDEgMCAwIDAgMS4yNy4wNmw0LjEyLTMuMTIgOS40NSA4LjYxYTEuNDkgMS40OSAwIDAgMCAxLjcuMjlsNC45NC0yLjM4QTEuNSAxLjUgMCAwIDAgMjQgMjAuMDZWMy45NGExLjUgMS41IDAgMCAwLS44NS0xLjM1Wk0xOCAxNy40NSAxMC44MyAxMiAxOCA2LjU1djEwLjlaIi8%2BPC9zdmc%2B&logoColor=white&color=007ACC)](https://marketplace.visualstudio.com/items?itemName=pascal-lab.vide-ide)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

Vide 是由南京大学 [PASCAL 研究组](https://pascal-lab.net/) 研发的完全开源现代 SystemVerilog 编程 IDE，旨在让硬件设计像软件开发一样流畅。Vide 为硬件开发者带来了十多项传统硬件 IDE 中常常缺失的代码分析能力，包括[定义跳转](https://vide.pascal-lab.net/user-guide/features/navigation/)、[代码注解](https://vide.pascal-lab.net/user-guide/features/annotations/)、[精准补全](https://vide.pascal-lab.net/user-guide/features/completion/)和[自动重构](https://vide.pascal-lab.net/user-guide/features/quick-fixes/)等。借助 Vide，硬件开发者能够更加高效地理解、编写和维护 Verilog/SystemVerilog 代码。

**功能展示、安装说明和用户手册请访问 [Vide 主页](https://vide.pascal-lab.net/)。**

构建语言服务器、VS Code 扩展和本地 VSIX 的步骤见 [从源码构建](https://vide.pascal-lab.net/advanced-guide/build-from-source/)。

如果你遇到问题或希望参与开发，请使用 GitHub Issues 和 Pull Requests。

## 许可证与致谢

Vide 使用 [MIT License](LICENSE)。

Vide 使用 [slang](https://github.com/MikePopoloski/slang) 提供 SystemVerilog 解析与诊断能力；slang 使用 MIT License。

Vide 的格式化功能可调用 [Verible](https://github.com/chipsalliance/verible) 的 `verible-verilog-format`；Verible 使用 Apache License 2.0。Verible 不随 Vide 打包，需要用户自行安装配置。
