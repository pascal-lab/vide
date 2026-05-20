---
title: 快速开始
description: 安装 Vizsla 扩展并确认核心 IDE 功能可用。
---

请根据以下步骤，快速开始体验 Vizsla。

## 1. 安装扩展

在 VS Code 的扩展面板中搜索显示名 `Vizsla` 并安装即可。

## 2. 打开工程目录

用 VS Code 打开包含 RTL 源码的目录。没有 `vizsla.toml` 或旧版 `vizsla_config.toml` 时, 扩展会创建默认 `vizsla.toml` 并弹出提示:

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/v1/vizsla.schema.json
# Syntax-only startup config. Keep these arrays empty to avoid scanning the workspace.
# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.
sources = []
include_dirs = []
```

这个默认清单不会扫描工程目录, 只启用打开文件的 syntax/parse diagnostics。

## 3. 确认状态栏

扩展激活后, 左侧状态栏会显示 Vizsla 服务器状态:

- `Vizsla Starting`: 正在启动。
- `Vizsla Ready`: 服务器已启动。
- `Vizsla Error`: 启动失败, 点击状态栏打开输出通道。
- `Vizsla Stopped`: 服务器已停止。

## 4. 打开 Verilog/SystemVerilog 文件

打开 `.v`, `.vh`, `.sv`, `.svh` 或 `.svi` 文件。VS Code 应该把它识别为 Verilog 或 SystemVerilog, 并启用语法高亮和语言服务。

## 5. 试用核心功能

你可以按这个顺序验证:

1. 写一处明显语法错误, 查看 `Problems` 面板中的诊断。
2. 在模块名、信号名或实例名上执行 `Go to Definition` 或 `Go to Declaration`。
3. 在实例端口连接、参数赋值、表达式或预处理位置触发补全。
4. 把光标放到符号上查看悬停说明。
5. 执行 `Format Document`。默认格式化会调用 `verible-verilog-format`, 如果本机没有这个工具, 请配置 `vizsla.formatter.path` 或先跳过格式化验证。

配置变更后, 如果 VS Code 提示重启语言服务器, 选择 `Restart`。

需要跨文件语义诊断、跳转和端口/参数相关能力时, 请在 `vizsla.toml` 中写入实际的 `sources` 或 `include_dirs`, 并按需补充 `defines`, `libraries` 或 `top_modules`。

推荐同时安装 [Tombi](https://github.com/tombi-toml/tombi) 作为 TOML formatter、linter 和 schema-aware completion。Vizsla 文档提供了可给 Tombi 关联的 `vizsla.toml` JSON Schema, 详见 [项目配置](./project-configuration.md#tombi-schema)。
