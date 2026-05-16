# 项目配置：vizsla_config.toml

小工程可以没有配置文件。大工程建议在工作区根目录放一个 `vizsla_config.toml`。这个文件告诉 Vizsla：哪些是源码、哪些是 include 目录、哪些宏要预定义、哪些目录不要管。

在你的工程根目录创建：

```text
vizsla_config.toml
```

注意文件名必须完全一致。

## 最小配置

如果你的工程是这样：

```text
my-chip/
  rtl/
    top.sv
    child.sv
  include/
    defs.svh
  tb/
    top_tb.sv
```

可以先写：

```toml
top_modules = ["top"]
sources = ["rtl"]
include_dirs = ["include"]
```

保存 `vizsla_config.toml` 后，Vizsla 会重新刷新工程信息。打开 `rtl/top.sv`，如果 include 和模块实例化能正常解析，说明配置生效。

## 完整字段

`vizsla_config.toml` 支持这些字段：

```toml
top_modules = ["top"]
defines = ["SYNTHESIS", "WIDTH=32"]
sources = ["rtl", "tb"]
include_dirs = ["include", "rtl"]
libraries = ["ip/vendor_lib"]
exclude = ["build", "target", "sim/out"]
```

字段含义如下：

- `top_modules`：顶层模块名列表。用于告诉 Vizsla 你的设计入口是什么。
- `defines`：预定义宏。可以写成 `FOO`，也可以写成 `FOO=bar`。
- `sources`：源码目录或源码文件。路径相对于 `vizsla_config.toml` 所在目录。
- `include_dirs`：查找 `` `include`` 文件时使用的目录。
- `libraries`：库文件或库目录。适合放第三方 IP、标准库或外部设计文件。如果路径指向另一个已经打开的工作区，Vizsla 会按显式依赖把那个工作区纳入当前工程分析。
- `exclude`：排除目录或文件。被排除的路径不会作为源码、include 或库参与分析。

## 路径规则

路径默认写相对路径。比如：

```toml
sources = ["rtl"]
include_dirs = ["include"]
```

这表示：

```text
工程根目录/rtl
工程根目录/include
```

你也可以写绝对路径，用来引用工程外的库：

```toml
libraries = ["D:/third_party/vendor_cells"]
```

> [!WARNING]
> **警告**
>
> `exclude` 会在路径转成绝对路径后生效。如果某个路径被排除了，即使它同时出现在 `sources`、`include_dirs` 或 `libraries` 里，Vizsla 也不会使用它。

## 没写 sources 时的默认行为

如果你没有写 `sources`，Vizsla 会默认把工作区根目录作为源码目录：

```toml
# 没有 sources
include_dirs = ["include"]
```

等价于把整个工程根目录交给 Vizsla 扫描。

> [!NOTE]
> **讨论**
>
> 这对小工程很方便，但对大工程可能太宽。生成目录、仿真输出、综合输出里可能有很多临时文件。大工程建议显式写 `sources` 和 `exclude`。

## 没写 include_dirs 时的默认行为

如果你没有写 `include_dirs`，Vizsla 会默认把 `sources` 当作 include 目录：

```toml
sources = ["rtl"]
# 没有 include_dirs
```

这表示 include 会在 `rtl` 里查找。

如果你明确写了空数组：

```toml
include_dirs = []
```

那就是告诉 Vizsla：不要自动使用任何 include 目录。

## 宏定义

没有值的宏这样写：

```toml
defines = ["SYNTHESIS"]
```

有值的宏这样写：

```toml
defines = ["WIDTH=32", "TECH=sky130"]
```

如果宏值里有空格，也可以直接写在等号后面：

```toml
defines = ["MESSAGE=hello world"]
```

打开使用这些宏的文件。如果之前因为宏未定义产生诊断，保存配置后诊断应该消失或变化。

## 常见工程模板

### 纯 RTL 工程

```toml
top_modules = ["top"]
sources = ["rtl"]
include_dirs = ["rtl", "include"]
exclude = ["build", "target"]
```

### RTL 加 testbench

```toml
top_modules = ["top_tb"]
sources = ["rtl", "tb"]
include_dirs = ["include", "rtl", "tb"]
defines = ["SIMULATION"]
exclude = ["sim/out", "build"]
```

### 带第三方 IP

```toml
top_modules = ["soc_top"]
sources = ["rtl"]
include_dirs = ["include", "ip/include"]
libraries = ["ip/vendor"]
exclude = ["ip/vendor/doc", "ip/vendor/examples", "build"]
```

### 多工作区依赖

如果 app 和 pkg 是并列工程，请在 VS Code 里同时打开这两个文件夹，并在 app 的配置里显式写出依赖：

```text
repo/
  app/
    vizsla_config.toml
    rtl/
  pkg/
    vizsla_config.toml
    include/
```

```toml
# app/vizsla_config.toml
top_modules = ["top"]
sources = ["rtl"]
include_dirs = ["../pkg/include"]
libraries = ["../pkg"]
```

`pkg` 仍然使用自己的 `vizsla_config.toml` 描述源码和 include 目录；`app` 通过 `libraries = ["../pkg"]` 明确声明依赖关系。

## 配置文件放在哪里

Vizsla 只会在你打开的工作区根目录寻找 `vizsla_config.toml`。它不会自动向父目录或子目录继续找。

请把配置文件放在 VS Code 打开的那个文件夹里：

```text
my-chip/
  vizsla_config.toml
  rtl/
  include/
```

> [!WARNING]
> **警告**
>
> 如果你在 VS Code 里打开的是 `my-chip/rtl`，但配置文件放在 `my-chip/vizsla_config.toml`，Vizsla 看不到这个配置。请重新打开 `my-chip`，或者把配置文件放到 `rtl` 里。

## 修改配置后怎么刷新

默认情况下，`vizsla.workspace.auto.reload` 是开启的。你保存 `vizsla_config.toml` 后，Vizsla 会自动刷新工程信息。

如果你觉得配置没有生效，可以手动运行：

```text
Vizsla: Restart Language Server
```

然后再看输出日志确认工程被重新加载。
