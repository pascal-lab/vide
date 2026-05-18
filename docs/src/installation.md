# 安装

## 通过 VS Code 扩展安装

普通用户只需要安装 VS Code 扩展。扩展会在启动时优先使用随扩展打包的 `vizsla` 服务器, 不需要你单独安装 Rust 或手动启动 LSP 进程。

扩展的显示名是 `Vizsla`, 扩展 ID 是 `vizsla.vizsla-lsp`。如果它已经出现在你的扩展来源中, 直接在 VS Code 扩展面板安装即可。

## 离线或本地 VSIX 安装

拿到 `.vsix` 文件后, 可以使用 VS Code 命令面板:

1. 打开命令面板。
2. 执行 `Extensions: Install from VSIX...`。
3. 选择对应平台的 `vizsla-vscode-*.vsix`。

也可以用命令行:

```powershell
code --install-extension .\vizsla-vscode-win32-x64.vsix
```

VSIX 是按平台打包的。当前打包脚本支持这些目标:

- `darwin-arm64`
- `linux-arm64`
- `linux-x64`
- `win32-x64`

## 什么时候配置自定义服务器

默认情况下不要配置 `vizsla.server.command`。扩展会在自己的安装目录下寻找 bundled server。

这些情况适合配置自定义服务器:

- 你从源码构建了 `vizsla`, 想让扩展使用本地二进制。
- 你正在调试服务器启动参数或日志。
- bundled server 缺失或不匹配当前平台。
- 你需要临时验证某个服务器版本。

推荐使用绝对路径:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe",
  "vizsla.server.args": [],
  "vizsla.server.cwd": "D:\\path\\to\\workspace",
  "vizsla.server.additionalArgs": ["--log", "debug"]
}
```

`vizsla.server.args` 和 `vizsla.server.additionalArgs` 都必须是字符串数组。扩展启动服务器时会先传 `server.args`, 再追加 `server.additionalArgs`。
