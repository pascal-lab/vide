# `hir-def` 外部架构比较：语法树、语义、增量与 IDE 查询

**研究范围。** 本文只记录可由一手资料定位的事实，并把解释和对 vide 的建议分开。资料访问日期统一为 **2026-08-06**。没有一手资料支持的结论标为“未证实”，不把实现推测写成事实。

## 1. 术语和比较边界

### 事实

- LSP 把编辑器请求建模为协议消息；位置型请求通常带有 `textDocument` 与 `position`，文档同步则通过 `TextDocumentContentChangeEvent` 表达全量或增量变更。LSP 3.17 规范还定义 `Location`/`LocationLink`、诊断、补全、语义 token 等结果形状。来源：[LSP 3.17 Specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)（章节锚点包括 `#textDocumentPositionParams`、`#textDocumentSyncKind`、`#textDocumentContentChangeEvent`、`#locationlink`；访问日期 2026-08-06）。
- 因此本文区分四层：**语法树**（容错解析和源码节点）、**语义模型**（定义/作用域/类型等）、**增量状态**（输入、缓存、失效和快照）、**IDE 查询接口**（把语义结果投影为编辑器偏移和协议数据）。这是比较框架，不是 LSP 规范对内部实现的强制要求。

### 解释

LSP 只规定服务器和客户端的可观察契约，不规定“一个文件一个 AST”“数据库 query”或“全局索引”。这些选择决定修改一个字符时要重建什么，也决定源码映射是否稳定。

## 2. rust-analyzer：分层查询与源码到定义的闭环

### 2.1 语法树与输入边界

#### 事实

- rust-analyzer 的架构文档说，解析器产出事件流，经 `TreeSink`/`TokenSource` 构造 `rowan` 树；解析不以失败的 `Result` 结束，而是产生树和错误集合。文档还明确 `syntax` 不依赖 salsa 或 LSP，树是由语法内容决定的值类型，并且允许不完整输入。来源：[rust-analyzer Architecture — `crates/parser`, `crates/syntax`](https://rust-analyzer.github.io/book/contributing/architecture.html#cratesparser) 和 [`#cratessyntax`](https://rust-analyzer.github.io/book/contributing/architecture.html#cratessyntax)（访问日期 2026-08-06）。
- 架构文档把客户端输入定义为源文件文本与 `CrateGraph` 等项目结构，分析器把输入保持在内存中；派生模型按需、懒惰计算，输入小改动后产生新模型。来源：[Bird’s Eye View](https://rust-analyzer.github.io/book/contributing/architecture.html#birds-eye-view)（访问日期 2026-08-06）。
- `base-db` 的官方源码把 `FileText`、`FileSourceRootInput`、`SourceRootInput` 定义为 salsa input，并以不透明 `FileId` 表示文件；`SourceDatabase` 暴露文件文本、source root 和 crate 图等数据库边界。来源：[rust-analyzer `crates/base-db/src/lib.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/base-db/src/lib.rs)（`FileText`、`SourceDatabase`；访问日期 2026-08-06）。

#### 解释

“语法独立于语义”是可复用边界：只需语法的格式化、结构导航和错误恢复，不必构建完整 crate。相反，`FileId`/source root 把文件系统和语义数据库隔离开，避免在每个语义查询里依赖路径 I/O。

### 2.2 salsa query、`ItemTree`、`DefMap` 和增量粒度

#### 事实

- 架构文档说明 `base-db` 使用 salsa；大多数输入 query 在 `base-db`，其余是派生 query。文档还说 `hir-expand`、`hir-def`、`hir-ty` 是编译器部分，深度集成 salsa/chalk；`ItemTree` 将单文件语法树压缩为对函数体修改稳定的摘要；`DefMap` 存储 crate 的模块树和模块作用域；`Body` 存储表达式。来源：[Architecture — `crates/base-db`](https://rust-analyzer.github.io/book/contributing/architecture.html#cratesbase-db) 与 [`#crateshir-expand-crateshir-def-crateshir_ty`](https://rust-analyzer.github.io/book/contributing/architecture.html#crateshir-expand-crateshir-def-crateshir_ty)（访问日期 2026-08-06）。
- 同一文档给出增量不变量：函数 `foo` 的函数体内部改动不应使关于 `bar` 的全局派生数据失效；`hir` 还按带特定 cfg 的 crate instance 解释同一份语法。来源同上（访问日期 2026-08-06）。
- 当前源码中的 `crate_local_def_map` 是 `#[salsa::tracked(returns(ref))]` query，返回 tracked 的 `DefMapPair`；它从 crate 根文件创建初始模块并调用 `collector::collect_defs`。`block_def_map` 也是 tracked query，针对 block 表达式创建独立 `DefMap`。来源：[rust-analyzer `crates/hir-def/src/nameres.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/nameres.rs)（`crate_local_def_map`、`block_def_map`；访问日期 2026-08-06）。
- `DefMap` 源码注释明确其结果包含 crate 模块树以及 item-level macro 展开后的各模块作用域；同一文件把计算分为收集 raw items、收集模块、解析 imports、解析 macros 的相互递归阶段。来源同上（文件顶部模块注释和 `DefMap` 定义；访问日期 2026-08-06）。
- `hir-def/src/lib.rs` 的模块注释把 `hir_def` 放在宏展开与类型推断之间，并导出 `item_tree`、`nameres`、`resolver`、`src` 等模块；同文件使用 `#[salsa::input]` 表示配置输入（例如 `ExpandProcAttrMacros`）。来源：[rust-analyzer `crates/hir-def/src/lib.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/lib.rs)（访问日期 2026-08-06）。

#### 解释

这里的“增量”不是只缓存 AST：输入文本 → parse/item tree → 宏/模块收集 → `DefMap` → body/type queries 的 query 图，才是失效边界。`ItemTree` 和 item/body 分离把最常见的函数体编辑限制在局部；`DefMap` 的 crate-instance/cfg 参数意味着“同名文件”不能单独作为语义键。

### 2.3 `DefMap`、`Resolver` 与名称解析

#### 事实

- `DefMap` 的 crate 版本以 crate 根为 root，记录模块、父子关系、可见性和 `ItemScope`；block expression 可以得到自己的 `DefMap`。来源：[nameres.rs](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/nameres.rs)（`DefMap`/`ModuleData`；访问日期 2026-08-06）。
- `Resolver` 是名称解析 façade。它持有从内到外排列的 scope 栈和模块 item map；scope 枚举包括 block/module scope、generic parameters、表达式局部绑定、body 内宏定义。类型和值 namespace 由 `TypeNs`、`ValueNs` 等枚举区分。来源：[rust-analyzer `crates/hir-def/src/resolver.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/resolver.rs)（访问日期 2026-08-06）。
- `resolve_path_in_type_ns_with_prefix_info` 按 scope 顺序处理 `Self`、泛型参数、模块 scope、builtin，并返回 namespace、未消费的 path segment、导入信息、前缀信息和可见性；这是当前源码中可定位的解析 API，而非只依赖文档概述。来源同上（访问日期 2026-08-06）。

#### 解释

把模块级解析（`DefMap`）和表达式级解析（`Resolver` 的 scope stack）分开，使“添加一个 import”与“修改函数局部绑定”拥有不同失效范围；namespace 分离也避免把类型和值名称空间强行压成单一字符串表。

### 2.4 源码映射：`AstId`、`InFile`、宏文件和 source-to-def

#### 事实

- `hir-def/src/src.rs` 的 `HasSource` 为带 `AstIdLoc` 的定义提供 `ast_ptr` 和 `source`：它以 `AstId` 查 `ast_id_map`，再在对应 real 或 expanded file 解析节点。`HasChildSource` 为字段、泛型参数、use tree 等提供从语义 child id 到 AST child 的映射。来源：[rust-analyzer `crates/hir-def/src/src.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/src.rs)（访问日期 2026-08-06）。
- `hir/src/semantics/source_to_def.rs` 明确描述了 syntax → def 的递归算法：先找语法容器，再递归取得容器 def，询问容器的 child defs，比较每个 child 的 source，找到与原节点相同者；文件节点处再通过 relevant crates 查找模块。来源：[rust-analyzer `crates/hir/src/semantics/source_to_def.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir/src/semantics/source_to_def.rs)（模块注释和 `SourceToDefCtx`；访问日期 2026-08-06）。
- 同一源码要求语法树用 `InFile` 携带 real/macro-expanded 文件身份；并明确由于 cfg 和 `#[path]`，syntax 到 def 不是单射，当前实现可能返回第一个可行答案。来源同上（访问日期 2026-08-06）。
- `hir/src/semantics.rs` 将 `Semantics` 作为语义入口，提供按 offset 找 AST、跨宏查找和 descend 到宏展开等接口；`SemanticsImpl` 持有 source-to-def、宏调用和匿名 const 的请求级缓存。来源：[rust-analyzer `crates/hir/src/semantics.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir/src/semantics.rs)（访问日期 2026-08-06）。

#### 解释

源码映射是 IDE 的关键反向边：声明、定义、引用、诊断最终都要回到文件和 range。r-a 不把语义对象直接塞进语法树；而是用带文件身份的 AST id/source 映射和“父 def → children → source”反查来处理一对多、宏展开和 cfg。映射不唯一不是偶发 bug，而是模型中必须显式表达的事实；“返回第一个”是当前维护策略，不应被当成普遍正确性保证。

### 2.5 IDE 查询接口

#### 事实

- 架构文档把 `hir` 描述为 API façade，把内部 ECS 风格 ID 包装成面向对象的静态、已解析视图；`ide` 进一步提供 completion、goto definition 等高层功能。来源：[Architecture — `crates/hir`](https://rust-analyzer.github.io/book/contributing/architecture.html#crateshir) 和 [`crates/ide`](https://rust-analyzer.github.io/book/contributing/architecture.html#crateside-crateside-db-crateside-assists-crateside-completion-crateside-diagnostics-crateside-ssr)（访问日期 2026-08-06）。
- `ide` 的公开 API 刻意使用可序列化的 POD 和 editor 术语（offset、range、label），通常不暴露 syntax tree 或 HIR 类型；`AnalysisHost` 可事务性 `apply_change`，`Analysis` 是不可变 snapshot。来源同上（访问日期 2026-08-06）。
- 架构文档规定只有 `rust-analyzer` 二进制 crate 了解 LSP/JSON，服务器层把 `ide` 数据转换成可序列化协议类型。来源：[Architecture — `crates/rust-analyzer`](https://rust-analyzer.github.io/book/contributing/architecture.html#cratesrust-analyzer)（访问日期 2026-08-06）。

#### 解释

这是“稳定 API 边界”：`hir-def` 可以为增量和语义正确性使用内部 ID，而 LSP/编辑器只见文件、range、文本和可序列化结果。建议不要让协议字段反向污染 `DefMap` 或 resolver 的数据结构。

## 3. clangd：编译器 AST + 每文件串行调度 + 全局索引

### 3.1 语法树、编译命令和 AST 生命周期

#### 事实

- clangd 官方设计文档说，它基于 clang 编译器，在每个打开文件上运行解析循环；解析器生成诊断并最终生成 clang AST，AST 被保存用于“光标下是什么符号”等查询。每个打开文件有一个循环，由 `TUScheduler` 管理多个 `ASTWorker`。来源：[clangd Design](https://clangd.llvm.org/design/)（访问日期 2026-08-06）。
- 每个文件解析需要虚拟 compiler command（语言、标准、include 路径等）；clangd 的 compile commands 文档说明该命令理想情况下来自构建系统。来源：[Compile commands](https://clangd.llvm.org/design/compile-commands)（访问日期 2026-08-06）。
- 官方线程设计说明 clang 支持把 include preamble 单独解析；preamble 构建可能很慢，更新后 AST 构建较快，preamble 是不可变且线程安全的字节，而 AST 本身即使只读也不是线程安全的。来源：[Threads and request handling](https://clangd.llvm.org/design/threads)（访问日期 2026-08-06）。

#### 解释

clangd 的增量单位首先是“一个 translation unit 的 preamble/AST”，不是通用依赖图 query。编译命令是语义输入的一部分：同一文本在不同 include/宏/语言标准下不是同一个语义模型。

### 3.2 请求调度和失效

#### 事实

- clangd 主线程解码 LSP 并派发到 `ClangdServer`；主线程不应阻塞，而是把操作放入 `TUScheduler`。每个 `ASTWorker` 为单文件维护队列，丢弃已取消读请求和被后续写覆盖的写请求；读操作可保证看到该队列此前的写入。来源：[clangd threads — Life of a request](https://clangd.llvm.org/design/threads#life-of-a-request)（访问日期 2026-08-06）。
- 写操作会重建 AST（必要时重建 preamble）并发布诊断；写入有 debounce，直到读取请求或短 deadline 才启动。补全是例外：它使用 clang completion API 做新解析，不等待最新 preamble。来源：[clangd threads — Debouncing / Code completion](https://clangd.llvm.org/design/threads#debouncing)（访问日期 2026-08-06）。
- 可定位的一手实现包括 [`clang-tools-extra/clangd/TUScheduler.cpp`](https://github.com/llvm/llvm-project/blob/main/clang-tools-extra/clangd/TUScheduler.cpp)、[`ParsedAST.cpp`](https://github.com/llvm/llvm-project/blob/main/clang-tools-extra/clangd/ParsedAST.cpp) 和 [`ClangdServer.cpp`](https://github.com/llvm/llvm-project/blob/main/clang-tools-extra/clangd/ClangdServer.cpp)（访问日期 2026-08-06）。

#### 解释

clangd 通过“每文件单写者 + 可丢弃过时工作”避免 AST 数据竞争，同时把用户输入期间的写合并。它没有直接对应 salsa 的任意 query 依赖传播；这是另一种明确的维护策略。

### 3.3 语义模型、源码位置和索引

#### 事实

- clangd 官方 index 文档将 index 定义为跨代码库的 symbols、refs、relations 数据库。`SymbolID` 合并同一实体的声明；`Ref` 是 symbol 到文件位置的边；`Relation` 是 symbol 之间带类型的边。`SymbolIndex` 是接口，`MergedIndex` 把多个 index 叠加给功能层。来源：[The clangd index](https://clangd.llvm.org/design/indexing)（访问日期 2026-08-06）。
- `FileIndex` 维护打开文件及其头文件的动态结果，保证编辑中的位置不陈旧；`BackgroundIndex` 在后台解析整个项目并缓存 `*.idx`；还可使用外部构建的 static index 或 remote index。来源同上（访问日期 2026-08-06）。
- 具体接口和实现可定位到 [`clang-tools-extra/clangd/index/Index.h`](https://github.com/llvm/llvm-project/blob/main/clang-tools-extra/clangd/index/Index.h)、[`FileIndex.cpp`](https://github.com/llvm/llvm-project/blob/main/clang-tools-extra/clangd/index/FileIndex.cpp)、[`BackgroundIndex.cpp`](https://github.com/llvm/llvm-project/blob/main/clang-tools-extra/clangd/index/Background.cpp)（仓库路径随 LLVM 版本可能移动；以上是官方 `main` 路径，访问日期 2026-08-06）。

#### 解释

clangd 把“当前文件高精度 AST”与“跨项目导航的近似/可持久化索引”分开。对 vide 而言，这提示全局引用搜索和当前文件 resolver 不必共享同一个昂贵表示；但 C/C++ 的声明/定义和 include 模型不能直接照搬到 Rust/Verilog 风格的模块模型。

## 4. gopls：Go 标准库语法/类型模型之上的快照和持久缓存

### 4.1 语法与语义前端

#### 事实

- gopls 原始官方设计文档说，gopls 作为长期运行的 editor backend，把定义、补全、诊断等能力放在同一进程，避免每个命令重复运行 type-checker；同时指出标准库 `go/token`、`go/ast`、`go/types` 更偏 compiler throughput，不支持增量修改、错误输入恢复和长期内存控制。来源：[gopls design](https://github.com/golang/tools/blob/master/gopls/doc/design/design.md)（“Context”“Inappropriate core functionality”；访问日期 2026-08-06）。
- 当前 gopls `Snapshot` 源码将快照定义为某个 view 的当前状态：它首先是一个在生命周期内对文件存在性/内容给出一致答案的 `file.Source`，还管理派生的 parsed files 和 packages；注释明确失效由 `Snapshot.clone` 负责。来源：[gopls `internal/cache/snapshot.go`](https://github.com/golang/tools/blob/master/gopls/internal/cache/snapshot.go)（`Snapshot` 注释和字段；访问日期 2026-08-06）。
- `Snapshot` 持有 metadata graph、file map、package handles、analysis cache keys，以及持久化 map/set；源码不变量要求 package handle 与 metadata 对齐，且已载入 package 的依赖也应载入（除非缺 import）。来源同上（访问日期 2026-08-06）。

#### 解释

gopls 选择复用 Go 编译器前端数据结构，再在服务层建立 snapshot、memoize、persistent map 和文件 overlay；这不是 salsa 式细粒度语义 query，但通过不可变/引用计数快照达到请求一致性。

### 4.2 文件→package 映射、缓存和持久化

#### 事实

- gopls 的原始设计文档明确指出核心 type-check 单位是 package，而编辑器变化单位是 file；文件内容变化还可能改变 package 归属（package 声明或 build tags），同一文件可能在多个 package 中。来源：[gopls design — Cache invalidation](https://github.com/golang/tools/blob/master/gopls/doc/design/design.md#cache-invalidation)（访问日期 2026-08-06）。
- 当前 `Session` 源码描述 session 持有 views、file contents、parse cache 和 memoized computations；`NewSession` 创建 overlay FS、最近解析缓存和 view map；session 关闭时等待 snapshots 释放。来源：[gopls `internal/cache/session.go`](https://github.com/golang/tools/blob/master/gopls/internal/cache/session.go)（`Session`、`NewSession`、`Shutdown`；访问日期 2026-08-06）。
- gopls 设计文档的历史“只内存缓存”决策已被官方当前源码/设计前言修正：前言说明随着 workspace 增长，gopls 采用内存缓存与磁盘索引的混合方案，并链接 Go 官方 scalability 博文。来源：[gopls design 前言](https://github.com/golang/tools/blob/master/gopls/doc/design/design.md) 与 [Go blog: gopls scalability](https://go.dev/blog/gopls-scalability)（访问日期 2026-08-06）。

#### 解释

gopls 的主要维护风险是 package 级依赖传播与 file 级编辑事件之间的映射；build tags 会改变输入集合，不能简单用“单文件 AST hash”替代 package identity。它证明了快照一致性和明确的 overlay 可以在非增量前端上构建可用的长期服务，但不证明这种方式能达到 r-a 的函数体局部失效粒度。

### 4.3 IDE 查询接口

#### 事实

- gopls 设计文档把 hover、definition、diagnostics 等功能映射到 LSP，并要求它们共享 AST/type 信息；文档也记录有些控制流、复杂重构等功能不适合现有 LSP，需要谨慎考虑协议扩展。来源：[gopls design — Features](https://github.com/golang/tools/blob/master/gopls/doc/design/design.md#features) 与 [Features not supported by LSP](https://github.com/golang/tools/blob/master/gopls/doc/design/design.md#features-not-supported-by-lsp)（访问日期 2026-08-06）。
- 当前 gopls 的 snapshot API 以 `ReadFile`、package handles、解析和 type-check 结果为服务层入口；这里的“IDE API 形态”由 gopls internal package 与 protocol 层组成，而非把 `go/types` 对象直接作为 LSP JSON。具体 request handler 路径可定位到 [`gopls/internal/server`](https://github.com/golang/tools/tree/master/gopls/internal/server)（官方仓库，访问日期 2026-08-06）。

#### 解释

gopls 说明协议层和语义层仍应分开，但它的 façade 围绕 package、URI 和 snapshot，而 r-a `ide` 更进一步把接口压到 POD/editor terminology。vide 应按自身查询需求设计 façade，不必复制任一具体类型系统。

## 5. Roslyn 的源码映射旁证（用于确认一个跨实现模式）

### 事实

- rust-analyzer 的官方 `source_to_def.rs` 注释直接指向 Roslyn 的一手实现 [`SyntaxTreeSemanticModel.cs`](https://github.com/dotnet/roslyn/blob/36a0c338d6621cc5fe34b79d414074a95a6a489c/src/Compilers/CSharp/Portable/Compilation/SyntaxTreeSemanticModel.cs#L1403-L1429)，并描述 `GetDeclaredType`：先取得父 syntax 的 symbol，再遍历父 symbol 的 children，按原始节点 text span 找匹配项。该描述与链接均来自 r-a 官方源码；访问日期 2026-08-06。
- Roslyn 的官方源码文件路径可直接定位到声明/语义模型实现：[Roslyn `SyntaxTreeSemanticModel.cs`](https://github.com/dotnet/roslyn/blob/main/src/Compilers/CSharp/Portable/Compilation/SyntaxTreeSemanticModel.cs)（访问日期 2026-08-06）。

### 解释

这里可确认的跨语言模式是“语法容器 → 语义父对象 → 子对象 source/span 反查”，而不是某种特定数据库。r-a 源码称 Kotlin 也有类似形状；本文不进一步断言 Kotlin 当前实现的性能或缓存策略，除非另查其官方源码。

## 6. 横向比较

| 维度 | rust-analyzer | clangd | gopls |
|---|---|---|---|
| 主要前端单位 | `rowan` 容错语法树；`ItemTree`/HIR 分层 | 每 TU 的 clang AST + preamble | Go `go/ast`/`go/types` 之上的 package；服务层 snapshot |
| 增量/一致性 | salsa tracked/input query，按依赖失效；`Analysis` snapshot | 每文件 `ASTWorker` 单写者、队列丢弃和 debounce | `Snapshot.clone`、overlay、memoize/persistent maps；package 依赖失效 |
| 名称/语义 | `DefMap` 模块 scope + `Resolver` scope stack；宏/cfg 纳入模型 | clang AST 查当前 TU；跨文件靠 SymbolIndex | package/type-check 及 build tags；package 是主要语义单位 |
| 跨文件导航 | semantic IDs + `HasSource`/`InFile`；可有多 crate 映射 | SymbolID、Ref、Relation 的 index | metadata/package handles、类型对象和 xrefs/索引 |
| 源码映射 | AST id/file id 与父子反查；宏展开显式 | clang SourceManager/AST locations（本文未单独展开其源码算法） | URI/offset 与 Go AST/type 信息（本文未断言内部唯一 ID 算法） |
| 协议边界 | `ide` POD；仅 server crate 处理 LSP/JSON | `ClangdLSPServer` → `ClangdServer` → scheduler | protocol/server → snapshot/cache |

表格中的每项都应以相应小节的官方来源为准；表格不是额外事实来源。

## 7. 对 vide `hir-def` 的可迁移原则

以下是**建议/解释**，不是外部实现事实；它们应与当前仓库代码地图合并后再决定：

1. **把输入、lowered summary、body、解析和 IDE façade 分开。** r-a 的 `ItemTree`/`DefMap` 说明顶层声明摘要可比完整 body 稳定；函数体编辑不应让不相关顶层定义全部失效。若 vide 的 slop 来自“所有节点共享一个粗粒度 lowered 值”，应优先拆出稳定声明索引与局部 body 数据。
2. **将语义身份绑定到配置/实例，而不是裸文件。** r-a 的 crate instance/cfg 及 gopls 的 build tags 都说明：同一文本在不同构建上下文可有不同定义集合。vide 的等价上下文（例如编译配置、设计单元/包边界）应出现在 query key 或显式 context 中。
3. **用单一、可追踪的符号 ID 和双向源码映射。** 保留 `semantic → source range` 与 `source → semantic` 两条路径；定义、字段、端口、局部绑定和展开/生成节点应明确映射失败和多重候选，而不是隐式依赖数组位置。r-a 的 `HasSource`/`HasChildSource` 与 `source_to_def` 是可迁移形状。
4. **把名称解析分成模块级和局部 scope。** `DefMap`/`Resolver` 的分层可避免每次局部编辑重算全局模块表；各 namespace（类型、值、宏或 vide 对应命名空间）应有显式优先级和可测试的不变量。
5. **为 IDE 查询建立 POD/editor-facing façade。** offset、range、label、symbol kind 等返回值与内部 arena/index 解耦；LSP JSON 只在协议适配层产生。这样可以替换 lowering 或缓存策略而不迁移所有 handlers。
6. **把快照和并发策略写成不变量。** 可借鉴 r-a 的 immutable `Analysis` 或 gopls 的 ref-counted snapshot，也可借鉴 clangd 每文件单写者；但必须明确一个查询看到哪一版输入、旧版本何时可释放、取消/过时工作是否丢弃。
7. **用来源范围作为第一等数据，而不是最后一步补 span。** clangd 的 index 与 r-a 的 source map 都把导航位置作为模型的一部分。对于生成/宏/展开节点，应存原始来源、生成来源和可展示范围的关系，避免在 IDE 层猜测。

## 8. 不可直接照搬的部分与未证实事项

- **不可照搬 salsa。** r-a 使用 salsa 是事实，但没有证据表明把 vide 所有 lowered 结构机械改成 tracked query 就会解决 slop；query 边界必须由 vide 的实际依赖图验证。
- **不可照搬 clangd 的 preamble/index。** preamble、include graph、C/C++ 声明/定义语义是 clang 的约束。clangd 的 FileIndex/BackgroundIndex 可借鉴“动态当前文件层 + 全局索引层”的分层思想，但其 SymbolID 合并规则不能直接作为 vide 的实体身份规则。
- **不可照搬 gopls 的 package 失效粒度。** gopls 官方明确 package 是 type-check 单位、file 是编辑单位；若 vide 的模块/设计单元更小，直接采用 package-wide 重建可能扩大 slop。
- **不要把 source-to-def 的“第一个可行答案”当成正确性保证。** r-a 官方源码明确把它描述为 cfg/`#[path]` 下的当前折衷；vide 若存在多设计单元、多配置或生成来源，应返回 context-specific 或多候选结果。
- **未证实：** 本文没有据一手来源确认 vide 当前 `hir-def` 的具体 slop 根因、查询耗时占比、缓存命中率或哪个 symbol map 是主分配热点；这些需要结合仓库代码地图、trace/profile 和针对性实验后才能断言。
- **未证实：** 本文没有断言 clangd 或 gopls 的全部源码映射细节、所有请求是否共享同一快照，或当前版本所有缓存格式的稳定性；相关小节只引用明确可定位的官方设计/源码。

## 9. 一手来源索引（访问日期均为 2026-08-06）

- rust-analyzer：[Architecture](https://rust-analyzer.github.io/book/contributing/architecture.html)、[`base-db/src/lib.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/base-db/src/lib.rs)、[`hir-def/src/lib.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/lib.rs)、[`hir-def/src/nameres.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/nameres.rs)、[`hir-def/src/resolver.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/resolver.rs)、[`hir-def/src/src.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir-def/src/src.rs)、[`hir/src/semantics.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir/src/semantics.rs)、[`hir/src/semantics/source_to_def.rs`](https://github.com/rust-lang/rust-analyzer/blob/master/crates/hir/src/semantics/source_to_def.rs)。
- clangd：[Design](https://clangd.llvm.org/design/)、[Threads](https://clangd.llvm.org/design/threads)、[Indexing](https://clangd.llvm.org/design/indexing)、[Compile commands](https://clangd.llvm.org/design/compile-commands)、[LLVM `TUScheduler.cpp`](https://github.com/llvm/llvm-project/blob/main/clang-tools-extra/clangd/TUScheduler.cpp)、[LLVM `Index.h`](https://github.com/llvm/llvm-project/blob/main/clang-tools-extra/clangd/index/Index.h)。
- gopls：[官方 design 文档](https://github.com/golang/tools/blob/master/gopls/doc/design/design.md)、[`internal/cache/session.go`](https://github.com/golang/tools/blob/master/gopls/internal/cache/session.go)、[`internal/cache/snapshot.go`](https://github.com/golang/tools/blob/master/gopls/internal/cache/snapshot.go)、[Go scalability blog](https://go.dev/blog/gopls-scalability)。
- Roslyn：[官方 `SyntaxTreeSemanticModel.cs`](https://github.com/dotnet/roslyn/blob/main/src/Compilers/CSharp/Portable/Compilation/SyntaxTreeSemanticModel.cs)。
- 协议：[LSP 3.17 Specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)。
