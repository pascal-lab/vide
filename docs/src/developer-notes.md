# 开发者说明

这一页说明仓库结构和常见改动入口。它不是 API 稳定性承诺，而是帮助你快速找到代码。

## 仓库结构

```text
.
  Cargo.toml
  src/
  crates/
  editors/vscode/
  docs/
```

主要目录：

- `src/`：顶层语言服务器入口、LSP 主循环、配置、协议转换。
- `crates/ide/`：编辑器功能实现，比如补全、诊断、跳转、格式化、code action。
- `crates/hir/`：语义模型和名称解析。
- `crates/syntax/`：语法层封装。
- `crates/project-model/`：工程配置和 `vizsla_config.toml` 解析。
- `crates/vfs/`、`crates/vfs-notify/`：虚拟文件系统和文件监听。
- `crates/slang/`：slang 相关代码和绑定。
- `editors/vscode/`：VS Code 扩展。
- `docs/`：这份 mdBook 用户手册。

## LSP 入口

语言服务器入口在：

```text
src/main.rs
```

这里负责：

1. 解析命令行参数。
2. 初始化日志。
3. 建立 stdio LSP 连接。
4. 读取 initialize params。
5. 构造 `Config`。
6. 返回 server capabilities。
7. 进入 main loop。

## 配置入口

用户配置在：

```text
src/config/user_config.rs
```

如果你新增 VS Code 设置，通常需要同时改：

1. `editors/vscode/package.json` 的 `contributes.configuration`。
2. `src/config/user_config.rs` 的 `UserConfig`。
3. 对应功能读取配置的位置。
4. `docs/src/vscode-settings.md`。

## LSP 请求处理

请求处理在：

```text
src/global_state/handlers/request.rs
```

这里把 LSP 参数转换成内部 `FileId`、`FilePosition`、`TextRange`，调用 `crates/ide`，再把结果转回 LSP 类型。

通知处理在：

```text
src/global_state/handlers/notification.rs
```

这里处理打开、修改、保存、关闭文件，配置变化，工作区变化，文件监听变化等事件。

## VS Code 扩展入口

扩展入口在：

```text
editors/vscode/src/extension.ts
```

这里负责：

1. 创建输出窗口。
2. 创建状态栏。
3. 注册三个命令。
4. 解析 VS Code 设置。
5. 找到 bundled server 或自定义 server。
6. 启动 `LanguageClient`。
7. 在配置变化后提示重启服务器。

平台判断在：

```text
editors/vscode/src/platform.ts
```

状态栏文案在：

```text
editors/vscode/src/status.ts
```

## 新增一个用户功能时怎么做

按这个顺序改：

1. 先在 `crates/ide` 实现纯 IDE 功能，并写 Rust 单元测试。
2. 在 `src/global_state/handlers/request.rs` 或 notification 处理层接入 LSP。
3. 在 `src/config/caps.rs` 暴露或调整 server capabilities。
4. 如果需要配置项，修改 `src/config/user_config.rs` 和 `editors/vscode/package.json`。
5. 如果 VS Code UI 也要变化，修改 `editors/vscode/src`。
6. 更新本手册对应章节。
7. 运行 `cargo test` 和 `npm test`。

用户能在 VS Code 里看到功能，日志没有错误，测试覆盖核心行为，文档说明了怎么开启、怎么使用、怎么排查。

