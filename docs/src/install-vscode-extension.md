# 从扩展市场安装

这一页只做一件事：把 `Vizsla LSP` 扩展从 VS Code 扩展市场装进 VS Code。

## 方法一：在扩展面板安装

1. 打开 VS Code。
2. 打开左侧 Extensions 面板。
3. 在搜索框输入 `Vizsla LSP`。
4. 找到我们发布的 Vizsla 扩展。
5. 点击 Install。

安装完成后，打开命令面板，输入 `Vizsla`。如果能看到这些命令，说明扩展已经装上了：

- `Vizsla: Show Language Server Output`
- `Vizsla: Restart Language Server`
- `Vizsla: Show Server Version`

## 方法二：用命令安装

如果你已经知道扩展 ID，可以在终端里运行：

```powershell
code --install-extension vizsla.vizsla-vscode
```

命令成功后，VS Code 会显示扩展安装完成。你也可以打开扩展面板，搜索 `Vizsla LSP`，确认它已经出现在已安装扩展里。

## 安装后发生了什么

扩展安装后会贡献 Verilog 和 SystemVerilog 语言支持，并在打开这些文件时启动 Vizsla 语言服务器。

Vizsla 识别这些文件后缀：

- `.v`
- `.vh`
- `.sv`
- `.svh`
- `.svi`

继续看 [打开你的第一个工程](./first-project.md)。如果你不是普通安装，而是要调试本地源码，请跳到 [从源码构建](./build-from-source.md)。

