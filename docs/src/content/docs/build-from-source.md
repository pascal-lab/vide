---
title: 从源码构建
description: 从源码构建 Vizsla 服务器、VS Code 扩展和本地 VSIX。
---

这一页面向需要本地开发、调试或打包 VSIX 的用户。

## 构建 Rust 服务器

在仓库根目录运行:

```powershell
cargo build
```

发布构建:

```powershell
cargo build --release
```

发布构建会把构建元数据写入 `vizsla --version` 输出。本地构建如果没有设置
`VIZSLA_COMMIT_HASH` 和 `VIZSLA_BUILD_DATE`, 会自动使用当前 Git 短提交和 UTC
构建时间; CI 或发布脚本仍然可以通过这两个环境变量覆盖默认值。

验证版本:

```powershell
.\target\release\vizsla.exe --version
```

非 Windows 平台使用:

```powershell
./target/release/vizsla --version
```

如果你只想让 VS Code 扩展使用本地构建的服务器, 配置:

```json
{
  "vizsla.server.command": "D:\\Proj\\vizsla\\target\\release\\vizsla.exe"
}
```

## 构建 VS Code 扩展

进入扩展目录:

```powershell
cd editors\vscode
npm install
npm run compile
```

`npm run compile` 会执行清理、TypeScript typecheck 和 esbuild bundle, 生成 `dist/extension.js`。

## 打包 VSIX

在 `editors\vscode` 下运行:

```powershell
npm run package
```

这个命令会:

1. 编译扩展。
2. 针对当前宿主平台执行 `cargo build --release`, 并使用同样的本地构建元数据默认值。
3. 把 `target/release/vizsla` 或 `vizsla.exe` 复制到扩展的 `server/<target>` 目录。
4. 临时把服务器二进制放到运行时 `server` 目录。
5. 调用 `vsce package --target <target>` 生成 `vizsla-vscode-<target>.vsix`。
6. 打包后清理临时运行时二进制。

你也可以指定目标:

```powershell
npm run package:win32-x64
npm run package:linux-x64
```

跨平台打包不会自动交叉编译 Rust 服务器。脚本要求目标平台的服务器二进制已经存在于 `editors/vscode/server/<target>/` 中, 或者你在匹配的原生 runner 上打包。

## 安装本地 VSIX

打包后可以运行:

```powershell
npm run install-extension
```

安装脚本会在当前目录查找 `vizsla-vscode-*.vsix`。如果有多个 VSIX 且未指定过滤词, 会安装最近修改的一个。

也可以直接:

```powershell
code --install-extension .\vizsla-vscode-win32-x64.vsix
```

这个命令要求 `code` 已经加入 `PATH`。
