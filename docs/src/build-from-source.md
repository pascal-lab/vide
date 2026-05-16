# 从源码构建

这一页给需要自己打包 VS Code 扩展、调试服务器、或参与开发的人看。如果你只是安装市场版本，可以跳过。

## 准备工具

安装这些工具：

1. Rust nightly。
2. Node.js 22 或更新版本。
3. npm。
4. VS Code。

在仓库根目录运行：

```powershell
rustc --version
cargo --version
node --version
npm --version
code --version
```

本仓库包含 `rust-toolchain.toml`，会选择 nightly toolchain。

## 构建语言服务器

在仓库根目录运行：

```powershell
cargo build
```

构建完成后，服务器在：

```text
target/debug/vizsla.exe
```

Windows 之外的平台文件名是：

```text
target/debug/vizsla
```

你可以查看版本：

```powershell
.\target\debug\vizsla.exe --version
```

## 编译 VS Code 扩展

进入扩展目录并安装依赖：

```powershell
cd editors\vscode
npm install
```

然后编译：

```powershell
npm run compile
```

编译会执行：

1. 清理旧输出。
2. TypeScript typecheck。
3. 用 esbuild 打包 `src/extension.ts` 到 `dist/extension.js`。

## 打包当前平台 VSIX

在 `editors/vscode` 目录运行：

```powershell
npm run package
```

打包脚本会：

1. 在仓库根目录执行 `cargo build --release`。
2. 把 `target/release/vizsla` 或 `vizsla.exe` 复制到扩展的 `server/` 目录。
3. 调用 `vsce package` 生成 VSIX。
4. 清理临时 staged runtime server 文件。

成功后会生成类似：

```text
editors/vscode/vizsla-vscode-win32-x64.vsix
```

## 打包指定平台

可用脚本：

```powershell
npm run package:darwin-arm64
npm run package:darwin-x64
npm run package:linux-x64
npm run package:linux-arm64
npm run package:win32-x64
npm run package:win32-arm64
```

> [!WARNING]
> **警告**
>
> 如果目标平台不是当前机器平台，打包脚本不会自动交叉编译服务器。它会要求你先把目标平台的服务器二进制放到：
>
> ```text
> editors/vscode/server/<target>/vizsla
> editors/vscode/server/<target>/vizsla.exe
> ```
>
> 例如 Linux x64：
>
> ```text
> editors/vscode/server/linux-x64/vizsla
> ```
>
> Windows x64：
>
> ```text
> editors/vscode/server/win32-x64/vizsla.exe
> ```

## 安装刚打包的 VSIX

在 `editors/vscode` 目录运行：

```powershell
npm run install-extension
```

如果目录里有多个 VSIX，脚本会安装最近修改的那个。你也可以传入过滤词：

```powershell
npm run install-extension -- win32-x64
```

安装后重新打开 VS Code，运行：

```text
Vizsla: Show Server Version
```

确认 VS Code 使用的是你刚打包的服务器。

## 不打包，直接调试本地服务器

先构建服务器：

```powershell
cargo build
```

然后在 VS Code 设置中写：

```json
{
  "vizsla.server.command": "D:/Proj/vizsla/target/debug/vizsla.exe",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

最后运行：

```text
Vizsla: Restart Language Server
```

输出窗口里的 `Server command` 应该指向 `target/debug/vizsla.exe`。

## 运行测试

Rust 测试：

```powershell
cargo test
```

VS Code 扩展测试：

```powershell
cd editors\vscode
npm test
```

> [!NOTE]
> **讨论**
>
> VS Code 扩展的 `npm test` 会先运行 `npm run compile`，再执行 `test/**/*.test.ts`。

## 预览文档

本手册使用 mdBook 0.5 内置的 admonitions。安装 `mdbook` 后运行：

```powershell
mdbook serve docs
```

