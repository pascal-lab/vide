---
title: 高级安装
description: 安装本地 VSIX、选择发布渠道，或配置自定义 Vide 语言服务器。
---

普通用户建议按 [VS Code 安装](../../user-guide/vscode-installation/) 安装 Marketplace 稳定版。本页用于离线安装、本地验证、预发布包和自定义服务器。

## 选择安装版本

除了 Marketplace，也可以下载 `.vsix` 文件后手动安装。按需要选择版本来源：

| 版本 | 获取方式 | 适合场景 |
| --- | --- | --- |
| 稳定版 | [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=pascal-lab.vide)，或 [GitHub Releases](https://github.com/pascal-lab/vide/releases) 中最新的正式 Release | 常规使用和离线安装 |
| Beta 版 | [GitHub Releases](https://github.com/pascal-lab/vide/releases) 中标记为 Pre-release 的发布 | 提前验证下一版功能 |
| Nightly 开发包 | [GitHub Actions CI](https://github.com/pascal-lab/vide/actions/workflows/ci.yml) 的运行产物，artifact 名称形如 `vide-vscode-dev-<target>-<commit>` | 验证某个提交或排查最新修复 |

VSIX 是按平台打包的。当前正式发布和 CI 产物覆盖这些目标：

- `alpine-arm64`
- `alpine-x64`
- `darwin-arm64`
- `linux-arm64`
- `linux-x64`
- `win32-x64`

## 安装 VSIX

拿到 `.vsix` 文件后，可以使用 VS Code 命令面板：

1. 打开命令面板。
2. 执行 `Extensions: Install from VSIX...`。
3. 选择对应平台的 `vide-vscode-*.vsix`。

也可以用命令行：

```powershell
code --install-extension .\vide-vscode-win32-x64.vsix
```

安装后如果状态栏报错，先按 [启动自检](../check-server/) 确认扩展找到的服务器路径和当前平台是否匹配。

## 配置自定义服务器

扩展默认使用随包提供的语言服务器。只有需要替换服务器二进制或调试启动参数时，才配置 `vide.server.command`。

这些情况适合配置自定义服务器：

- 你从源码构建了 `vide`，想让扩展使用本地二进制。
- 你正在调试服务器启动参数或日志。
- 随扩展打包的服务器缺失或不匹配当前平台。
- 你需要临时验证某个服务器版本。

推荐使用绝对路径：

```json
{
  "vide.server.command": "D:\\tools\\vide\\vide.exe",
  "vide.server.args": [],
  "vide.server.cwd": "D:\\path\\to\\workspace",
  "vide.server.additionalArgs": ["--log", "debug"]
}
```

`vide.server.args` 和 `vide.server.additionalArgs` 都必须是字符串数组。扩展启动服务器时会先传 `server.args`，再追加 `server.additionalArgs`。完整配置项见 [VS Code 设置参考](../../user-guide/vscode-settings/#server)。
