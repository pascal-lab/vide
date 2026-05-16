# 安装方式：扩展市场或源码构建

在动手之前，我们先把安装路径分清楚。Vizsla 有两个入口：

- **扩展市场安装**：适合绝大多数用户。你只需要在 VS Code 里安装 `Vizsla LSP` 扩展，扩展会负责启动随包发布的语言服务器。
- **源码构建**：适合项目开发者，或者想试用尚未发布的本地改动。你需要自己编译 Rust 语言服务器，并编译或打包 VS Code 扩展。

如果你只是写 Verilog/SystemVerilog，请走扩展市场安装。如果你要改 Vizsla 本身，请走源码构建。

## 你需要准备什么

### 扩展市场安装

先确认电脑上已经安装 Visual Studio Code 1.101.0 或更新版本。

打开终端，运行：

```powershell
code --version
```

如果能看到版本号，说明 VS Code 命令行入口已经可用。即使命令行入口暂时不可用，你也可以直接在 VS Code 的扩展面板里搜索并安装。

> [!WARNING]
> **警告**
>
> 如果系统提示找不到 `code`，不要急着改 Vizsla。先在 VS Code 里打开命令面板，执行 `Shell Command: Install 'code' command in PATH`。Windows 上通常安装 VS Code 后已经有这个命令；如果没有，请重新安装 VS Code，并勾选把 VS Code 加入 PATH 的选项。

### 源码构建

如果你要从源码构建，请准备：

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

这些命令都能输出版本号后，再继续看 [从源码构建](./build-from-source.md)。

## 该选哪条路

如果你看到的是这类目标，请选扩展市场安装：

- 我想让 VS Code 支持 Verilog/SystemVerilog。
- 我想在项目里用补全、跳转、诊断、格式化。
- 我不打算修改 Vizsla 源码。

如果你看到的是这类目标，请选源码构建：

- 我想改 Vizsla 的语言服务器。
- 我想调试 VS Code 扩展。
- 我想验证本地分支里的新功能。
- 扩展市场还没有发布我需要的版本。

普通用户继续看 [从扩展市场安装](./install-vscode-extension.md)。开发者直接看 [从源码构建](./build-from-source.md)。

