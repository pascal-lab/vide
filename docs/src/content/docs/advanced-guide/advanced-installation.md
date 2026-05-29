---
title: 高级安装
description: 从源码构建 Vide、打包本地 VSIX，或安装预发布版本。
---

## 引言

我们建议用户按 [VS Code 安装](../../user-guide/vscode-installation/) 直接从 VS Code Marketplace 安装稳定版。但是：

- 如果你想基于或修改源码来安装，可以查看 [从源码构建安装](#build-from-source-installation)。
- 如果你想体验预发布版本，可以查看 [从预发布版本安装](#prerelease-installation)。

<a id="build-from-source-installation"></a>

## 从源码构建安装

本节面向想基于或修改源码来安装 Vide 的用户。

### 环境要求

如果你只需要构建 Vide 语言服务器，请准备：

- Rust Nightly Toolchain
- CMake 3.20 至 3.29 之间的版本
- Python 3
- 一套支持 C++20 的编译器
  - Linux: GCC 11 或更新版本，或 Clang 16 或更新版本
  - macOS: Xcode 15 或更新版本
  - Windows: Visual Studio 2022 Build Tools 最新更新版，并勾选 `Desktop development with C++`

如果你还要构建 VS Code 扩展或打包 VSIX，请额外准备：

- Node.js 22.x
- npm：直接使用 Node.js 22 自带的 npm 即可

如果你还要构建 Playground 使用的 WASM 版本，请额外准备：

- Emscripten SDK 5.0.2
- `ninja`
- Rust `wasm32-unknown-emscripten` target
  - `playground/scripts/build-vide-wasm.mjs` 会自动执行 `rustup target add wasm32-unknown-emscripten`

### 构建 Vide 语言服务器

Vide 的核心是一个用 Rust 编写的语言服务器。VS Code 中的代码导航、补全、悬停、重命名、诊断等语义能力主要由这个语言服务器提供；扩展负责启动服务器、与之通信，并把结果接入编辑器界面。

要构建这个语言服务器，先在仓库根目录运行：

```bash
cargo build
```

如果你本地已经安装了 Vide 的 VS Code 扩展，可以先通过 VS Code 设置，直接让它使用刚才编译的语言服务器。上面的 `cargo build` 会生成 debug 版本，因此这里应指向 `target/debug`：

```json
{
  "vide.server.command": "D:/Proj/vizsla/target/debug/vide.exe"
}
```

如果你改用 `cargo build --release`，把路径改成 `D:/Proj/vizsla/target/release/vide.exe` 即可。

保存后 VS Code 会提示 `重启`；接受提示后可用 `Vide：显示服务器版本` 验证扩展实际使用的二进制。如果你还需要传启动参数或设置工作目录，完整字段说明见 [VS Code 设置参考](../../user-guide/vscode-settings/#server)。

当然，你也可以继续按照下面的过程构建一个完整的 VS Code 插件并安装。

### 构建 VS Code 扩展

进入 VS Code 扩展目录，首先编译：

```bash
cd editors/vscode
npm ci
npm run compile
```

`npm run compile` 会做以下事情：

1. 清理 `out` 和 `dist`，并执行 TypeScript typecheck。
2. 用 esbuild 把 `src/extension.ts` 打包到 `dist/extension.js`。
3. 把诊断性能分析视图需要的 Speedscope 静态资源复制到 `dist/speedscope`。

### 打包 VSIX

如果你只想在本机调试，或者要打包一个带调试信息的 VSIX，在 `editors/vscode` 下运行：

```bash
npm run package:debug
```

这个命令会：

1. 编译扩展，所以前面没手动执行 `npm run compile` 也可以。
2. 针对当前宿主平台执行 `cargo build`。
3. 把 `target/debug/vide` 或 `vide.exe` 复制到扩展的 `server/<target>` 目录。
4. 临时把服务器二进制放到运行时 `server` 目录。
5. 调用 `vsce package --target <target>` 生成 `vide-vscode-<target>-debug.vsix`。
6. 打包后清理临时运行时二进制。

如果你要打包能安装特定平台发布版 Vide 的 VSIX，可以运行以下一个或多个命令：

```bash
npm run package:linux-x64
npm run package:linux-arm64
npm run package:win32-x64
npm run package:darwin-arm64
npm run package:alpine-x64
npm run package:alpine-arm64
```

这些脚本会先编译扩展，然后准备目标平台的 release 版语言服务器，再生成 `vide-vscode-<target>.vsix`。当前 release workflow 只覆盖上面这些目标：glibc Linux、Windows x64、macOS arm64，以及 Alpine/musl x64 和 arm64。
这几项也是当前 CI 会实际构建的 VSIX 目标。其他平台即使在 `package.json` 里有脚本入口，也不表示它们在本地或当前 workflow 里一定能直接打包成功。

语言服务器的准备规则由 `editors/vscode/scripts/package.ts` 决定：

- 目标等于当前宿主平台时，脚本执行 `cargo build --release` 并复制产物。
- Alpine 目标在 CI 的 musl 容器中构建；本地脚本会添加对应 Rust musl target，但仍需要可用的 musl 交叉编译环境。
- 其他非宿主平台目标不会自动交叉编译语言服务器，需要 `editors/vscode/server/<target>/` 下已经存在对应的 `vide` 或 `vide.exe`，或者在匹配的原生 runner 上打包。

### 安装 VS Code 插件

打包后可以运行：

```bash
npm run install-extension
```

安装脚本会在当前目录查找 `vide-vscode-*.vsix`。如果有多个 VSIX，会安装最近修改的一个。也可以传入文件名片段来选择特定 VSIX：

```bash
npm run install-extension -- win32-x64-debug
```

也可以直接：

```bash
code --install-extension ./vide-vscode-win32-x64-debug.vsix
```

这个命令要求 `code` 已经加入 `PATH`。

<a id="prerelease-installation"></a>

## 从预发布版本安装

从预发布版本安装可以提前体验 Vide 的 Beta 特性。在安装前，你需要先拿到 `.vsix` 安装文件。

### 选择安装版本

除了 Marketplace，也可以下载 `.vsix` 文件后手动安装。按需要选择版本来源：

| 版本 | 获取方式 | 适合场景 |
| --- | --- | --- |
| 稳定版 | [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=pascal-lab.vide-ide)，或 [GitHub Releases](https://github.com/pascal-lab/vide/releases) 中最新的正式 Release | 常规使用和离线安装 |
| Beta 版 | [GitHub Releases](https://github.com/pascal-lab/vide/releases) 中标记为 Pre-release 的发布 | 提前验证下一版功能 |
| Nightly 开发包 | [GitHub Actions CI](https://github.com/pascal-lab/vide/actions/workflows/ci.yml) 的运行产物，artifact 名称形如 `vide-vscode-dev-<target>-<commit>` | 验证某个提交或排查最新修复 |

VSIX 是按平台打包的。当前正式发布和 CI 产物覆盖这些目标：

- `alpine-arm64`
- `alpine-x64`
- `darwin-arm64`
- `linux-arm64`
- `linux-x64`
- `win32-x64`

### 安装 VSIX

拿到 `.vsix` 文件后，可以使用 VS Code 命令面板：

1. 打开命令面板。
2. 执行 `Extensions: Install from VSIX...`。
3. 选择对应平台的 `vide-vscode-*.vsix`。

也可以直接把 `.vsix` 文件拖到 VS Code 的扩展面板中。安装成功后，右下角会出现已安装提示。

也可以用命令行：

```powershell
code --install-extension ./vide-vscode-win32-x64.vsix
```

安装后如果状态栏报错，先看 [故障报告与排查](../troubleshooting/) 确认扩展找到的服务器路径和当前平台是否匹配。
