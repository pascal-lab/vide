---
title: 第一个工程
description: 为 Vizsla 准备第一个 Verilog/SystemVerilog 工程目录。
---

## 推荐目录

小工程可以先用简单结构：

```text
my-rtl/
  rtl/
    top.sv
    alu.sv
  include/
    defs.svh
```

直接用 VS Code 打开 `my-rtl`。这个目录就是工作区根目录，也就是 VS Code 当前打开的顶层目录：

```powershell
code D:\work\my-rtl
```

然后打开 Verilog `.v`/`.vh` 文件，或 SystemVerilog `.sv`/`.svh`/`.svi` 文件。安装扩展后，VS Code 应该识别语言、显示语法高亮，并启动 Vizsla 的后台服务。

## 没有项目配置文件时会怎样

如果工作区根目录下有 Verilog/SystemVerilog 文件，但没有 `vizsla.toml` 或旧版且已弃用的 `vizsla_config.toml`，扩展会提示创建默认 `vizsla.toml`。选择创建后，扩展会写入下面的文件并重新加载 Vizsla：

```toml
#:schema https://pascal-lab.github.io/vizsla/schemas/v1/vizsla.schema.json
sources = []

# include_dirs = ["include"]
# defines = ["SYNTHESIS"]
# top_modules = ["top"]
# libraries = ["../common_cells"]
# exclude = ["build/**"]
```

这个默认项目配置文件写入 `sources = []`，意思是：先不要自动扫描整个工作区。这样可以先安全打开工程、确认扩展能启动，再由你决定哪些目录应该纳入项目分析。

如果你手写 `vizsla.toml` 但不写 `sources`，Vizsla 会尽力索引工作区里的 Verilog/SystemVerilog 文件。尽力索引可以让跳转、引用、悬停等阅读功能尽量可用，但它仍然不等于完整工程配置。显式写入 `sources = []` 则表示不要自动扫描工作区。

头文件（`.vh`、`.svh`、`.svi`）通常通过被 `.v` 或 `.sv` 文件 include 后参与诊断。只打开一个头文件，或者只把目录写进 `include_dirs`，不一定会得到独立的头文件诊断。

## 什么时候需要编辑项目配置文件

可以先按上面的方式跑起来。出现这些需求时，再编辑工作区根目录下的 `vizsla.toml`：

- 你希望跨文件跳转、引用、补全和诊断更接近真实工程。
- 你只想扫描 `rtl` 和 `include`，不想扫描仿真输出、生成目录或第三方缓存。
- 你需要设置 `defines`。
- 你需要告诉 Vizsla include 文件在哪里。
- 你有外部库目录，希望它们作为依赖参与分析。
- 你想显式声明 `top_modules`。

一个常见的小工程可以这样配置：

```toml
top_modules = ["top"]
defines = ["SYNTHESIS", "DATA_WIDTH=32"]
sources = ["rtl/**"]
include_dirs = ["include"]
exclude = ["build/**", "out/**"]
```

项目配置文件只会从你打开的工作区根目录读取。Vizsla 不会自动向父目录或子目录搜索其它项目配置文件；如果推荐的 `vizsla.toml` 和旧版 `vizsla_config.toml` 同时存在，会优先读取 `vizsla.toml`。`vizsla_config.toml` 仍兼容旧工程，但已弃用。

下一步可以阅读 [项目配置](./project-configuration.md)，把 `sources`、`include_dirs`、`defines` 和排除规则按你的工程结构写清楚。
