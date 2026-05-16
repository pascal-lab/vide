# Vizsla

Vizsla 是一个面向 Verilog 和 SystemVerilog 的语言服务器，以及配套的 VS Code 扩展。我们把日常写 RTL 时最常用的能力放进编辑器里：语法高亮、诊断、跳转、补全、悬停说明、引用查找、重命名、格式化、代码操作、语义高亮、折叠、符号大纲、签名帮助、inlay hints 和实例 code lens。

如果你只是想把 Vizsla 用起来，请从用户手册开始读：

- [Vizsla 用户手册](docs/src/SUMMARY.md)
- [快速上手](docs/src/quick-start.md)
- [项目配置](docs/src/project-configuration.md)
- [VS Code 设置参考](docs/src/vscode-settings.md)

## 30 秒认识 Vizsla

Vizsla 由两部分组成：

1. `vizsla`：Rust 编写的 LSP 服务器，负责理解 Verilog/SystemVerilog 工程。
2. `editors/vscode`：VS Code 扩展，负责启动服务器并把功能接到编辑器界面上。

普通用户安装 VS Code 扩展即可。扩展会随发布包带上对应平台的语言服务器；只有从源码开发、调试，或者想使用自己编译的服务器时，才需要手动配置 `vizsla.server.command`。

## 安装方式

普通用户请从 VS Code 扩展市场安装：

```powershell
code --install-extension vizsla.vizsla-vscode
```

也可以在 VS Code 扩展面板中搜索 `Vizsla LSP` 并点击 Install。

如果你要修改 Vizsla、调试本地服务器，或者扩展还没有发布到市场，请看 [从源码构建](docs/src/build-from-source.md)。

## 文档本地预览

本项目的完整文档使用 mdBook 组织，并使用 mdBook 0.5 内置的 admonitions 渲染提示框。安装 `mdbook` 后，可以在仓库根目录运行：

```powershell
mdbook serve docs
```

浏览器打开命令输出里的地址即可阅读。

## 许可证

Vizsla 使用 MIT License。
