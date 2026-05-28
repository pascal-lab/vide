---
title: 从源码构建
description: 从源码构建 Vide 语言服务器、VS Code 扩展和本地 VSIX。
---

本页面面向需要本地构建服务器、调试扩展启动或打包 VSIX 的用户。构建完成后，扩展启动检查请看 [当扩展无法正常启动](../check-server/)。

## 环境要求

从源码构建 Vide 时，`cargo build` 会通过 Rust 构建脚本编译仓库内的
`crates/slang`，因此除了 Rust 之外还需要能编译 slang 的 C++ 环境：

- Rust 工具链和 Cargo。
- CMake 3.20 或更新版本。
- Python 解释器，供 slang 的 CMake 配置阶段使用。
- 支持 C++20 的 C++ 编译器。Windows 建议安装 Visual Studio 2022 Build Tools，
  并选择 "Desktop development with C++" 组件；Linux/macOS 建议使用较新的
  GCC 或 Clang，其中 slang 至少需要 GCC 10 级别的 C++20 支持。
- Node.js 和 npm，用于构建 VS Code 扩展与打包 VSIX。

Vide 使用仓库内置的 slang 源码。构建 Vide 语言服务器和打包 VSIX 时，
构建脚本会一起编译这部分代码。

## 构建 Vide 语言服务器

在仓库根目录运行：

```powershell
cargo build
```

发布构建：

```powershell
cargo build --release
```

`vide --version` 默认只包含包版本和构建 profile，不会自动读取当前 Git
提交。需要给 beta、nightly 或内部构建加额外标记时，可以显式设置
`VIDE_BUILD_METADATA`：

```powershell
$env:VIDE_BUILD_METADATA = "abc1234.20260529T120000Z"
cargo build --release
```

验证版本：

```powershell
.\target\release\vide.exe --version
```

非 Windows 平台使用：

```powershell
./target/release/vide --version
```

让 VS Code 扩展使用本地构建的语言服务器时，配置：

```json
{
  "vide.server.command": "D:\\Proj\\vide\\target\\release\\vide.exe"
}
```

保存后 VS Code 会提示 `重启`；接受提示后可用 `Vide：显示服务器版本` 验证扩展实际使用的二进制。

## 构建 VS Code 扩展

进入扩展目录：

```powershell
cd editors\vscode
npm ci
npm run compile
```

`npm run compile` 只构建扩展本身：它会清理 `out` 和 `dist`，执行
TypeScript typecheck，用 esbuild 打包 `src/extension.ts` 到
`dist/extension.js`，并把诊断性能分析视图需要的 Speedscope 静态资源复制到
`dist/speedscope`。这个步骤不会构建或复制服务器二进制。

## 打包 VSIX

如果只是本机调试 VSIX，在 `editors\vscode` 下运行：

```powershell
npm run package:debug
```

这个命令会：

1. 编译扩展。
2. 针对当前宿主平台执行 `cargo build`。
3. 把 `target/debug/vide` 或 `vide.exe` 复制到扩展的 `server/<target>` 目录。
4. 临时把服务器二进制放到运行时 `server` 目录。
5. 调用 `vsce package --target <target>` 生成 `vide-vscode-<target>-debug.vsix`。
6. 打包后清理临时运行时二进制。

发布流程当前会产出这些平台的 release VSIX：

```powershell
npm run package:linux-x64
npm run package:linux-arm64
npm run package:win32-x64
npm run package:darwin-arm64
npm run package:alpine-x64
npm run package:alpine-arm64
```

这些脚本会先编译扩展，然后准备目标平台的 release 版语言服务器，再生成
`vide-vscode-<target>.vsix`。当前 release workflow 只覆盖上面这些目标：
glibc Linux、Windows x64、macOS arm64，以及 Alpine/musl x64 和 arm64。

`package.json` 里也保留了 `package:win32-arm64` 和 `package:darwin-x64`
入口，供手动验证或未来 release matrix 扩展使用；当前 release 流程不会
发布这两个目标的官方 VSIX。

语言服务器的准备规则由 `editors/vscode/scripts/package.ts` 决定：

- 目标等于当前宿主平台时，脚本执行 `cargo build --release` 并复制产物。
- Alpine 目标在 CI 的 musl 容器中构建；本地脚本会添加对应 Rust musl
  target，但仍需要可用的 musl 交叉编译环境。
- 其他非宿主平台目标不会自动交叉编译语言服务器，需要
  `editors/vscode/server/<target>/` 下已经存在对应的 `vide` 或 `vide.exe`，
  或者在匹配的原生 runner 上打包。

## 安装本地 VSIX

打包后可以运行：

```powershell
npm run install-extension
```

安装脚本会在当前目录查找 `vide-vscode-*.vsix`。如果有多个 VSIX 且未指定过滤词，会安装最近修改的一个。
也可以传入文件名片段来选择特定 VSIX：

```powershell
npm run install-extension -- win32-x64-debug
```

也可以直接：

```powershell
code --install-extension .\vide-vscode-win32-x64-debug.vsix
```

这个命令要求 `code` 已经加入 `PATH`。
