# 第一个工程

## 推荐目录

小工程可以先用简单结构:

```text
my-rtl/
  rtl/
    top.sv
    alu.sv
  include/
    defs.svh
```

直接用 VS Code 打开 `my-rtl`:

```powershell
code D:\work\my-rtl
```

## 没有清单时会怎样

如果 VS Code 打开的 workspace root 下没有 `vizsla_config.toml`, 扩展会创建一个默认清单并弹出提示。默认清单等价于:

```toml
sources = ["."]
include_dirs = ["."]
```

这适合源文件集中、目录不大的工程。你可以之后再按实际目录改成 `rtl`, `include` 或其它路径。

如果通过其它客户端或命令行方式启动服务器, 且确实没有 `vizsla_config.toml`, Vizsla 只保留 syntax/parse diagnostics, 不运行 semantic diagnostics。

## 什么时候创建清单

当工程变大时, 我们建议编辑 workspace root 下的 `vizsla_config.toml`:

- 你只想扫描 `rtl` 和 `include`, 不想扫描仿真输出、生成目录或第三方缓存。
- 你需要设置 `defines`。
- 你需要把 include 目录和 source 目录分开。
- 你有外部库目录, 希望它们作为依赖参与分析。
- 你想显式声明 `top_modules`。

示例:

```toml
top_modules = ["top"]
defines = ["SYNTHESIS", "DATA_WIDTH=32"]
sources = ["rtl"]
include_dirs = ["include"]
exclude = ["build", "out"]
```

清单只会从你打开的 workspace root 读取。我们不会自动向父目录或子目录搜索其它 `vizsla_config.toml`。
