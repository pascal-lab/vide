<p align="center">
  <a href="https://vide.pascal-lab.net/">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/pascal-lab/vide/master/docs/src/assets/vide-logo-dark.png">
      <img src="https://raw.githubusercontent.com/pascal-lab/vide/master/docs/src/assets/vide-logo-light.png" alt="Vide" width="520">
    </picture>
  </a>
</p>

# Vide - Verilog/SystemVerilog Coding IDE

[![Homepage](https://img.shields.io/badge/homepage-vide.pascal--lab.net-0969da)](https://vide.pascal-lab.net/en/)
[![Playground](https://img.shields.io/badge/playground-try%20online-7c3aed)](https://vide.pascal-lab.net/en/playground/)
[![中文 README](https://img.shields.io/badge/README-%E4%B8%AD%E6%96%87-d73a31)](README.zh-CN.md)
[![CI](https://github.com/pascal-lab/vide/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/pascal-lab/vide/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/pascal-lab/vide?sort=semver&color=2ea44f)](https://github.com/pascal-lab/vide/releases)
[![VS Code Marketplace](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fpascal-lab.gallery.vsassets.io%2F_apis%2Fpublic%2Fgallery%2Fpublisher%2Fpascal-lab%2Fextension%2Fvide-ide%2Flatest%2Fassetbyname%2FMicrosoft.VisualStudio.Code.Manifest&query=%24.version&label=Marketplace&prefix=v&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTIzLjE1IDIuNTkgMTguMjEuMjFhMS40OSAxLjQ5IDAgMCAwLTEuNy4yOUw3LjA2IDkuMTEgMi45NCA1Ljk5YTEgMSAwIDAgMC0xLjI3LjA2TC4zMyA3LjI2YTEgMSAwIDAgMCAwIDEuNDhMMy45IDEyIC4zMyAxNS4yNmExIDEgMCAwIDAgMCAxLjQ4bDEuMzQgMS4yMWExIDEgMCAwIDAgMS4yNy4wNmw0LjEyLTMuMTIgOS40NSA4LjYxYTEuNDkgMS40OSAwIDAgMCAxLjcuMjlsNC45NC0yLjM4QTEuNSAxLjUgMCAwIDAgMjQgMjAuMDZWMy45NGExLjUgMS41IDAgMCAwLS44NS0xLjM1Wk0xOCAxNy40NSAxMC44MyAxMiAxOCA2LjU1djEwLjlaIi8%2BPC9zdmc%2B&logoColor=white&color=007ACC)](https://marketplace.visualstudio.com/items?itemName=pascal-lab.vide-ide)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

Vide is a modern coding IDE for Verilog/SystemVerilog developers, designed to make hardware design feel as fluid as software development. Vide brings IDE-standard code analysis features that hardware IDEs often lack, including [definition navigation](https://vide.pascal-lab.net/en/user-guide/features/navigation/), [annotations](https://vide.pascal-lab.net/en/user-guide/features/annotations/), [precise completion](https://vide.pascal-lab.net/en/user-guide/features/completion/), and [automatic refactoring](https://vide.pascal-lab.net/en/user-guide/features/quick-fixes/). With Vide, hardware developers can understand, write, and maintain Verilog/SystemVerilog code more efficiently.

**Feature demos, installation guides, and the user manual are available on the [Vide homepage](https://vide.pascal-lab.net/en/).**

Build instructions for the language server, VS Code extension, and local VSIX packages are available in [Build from Source](https://vide.pascal-lab.net/en/advanced-guide/build-from-source/).

If you run into problems or want to contribute, please use GitHub Issues and Pull Requests.

## License and Acknowledgements

Vide is licensed under the [MIT License](LICENSE).

Vide uses [slang](https://github.com/MikePopoloski/slang) for SystemVerilog parsing and diagnostics. slang is licensed under the MIT License.

Vide's formatting feature can call [Verible](https://github.com/chipsalliance/verible) `verible-verilog-format`. Verible is licensed under Apache License 2.0. Verible is not bundled with Vide and must be installed and configured separately.
