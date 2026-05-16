# 打开你的第一个工程

Vizsla 以 VS Code 的工作区文件夹作为工程入口。你打开哪个文件夹，它就从哪个文件夹开始看你的 Verilog/SystemVerilog 文件。

## 准备一个最小工程

新建一个文件夹，比如 `vizsla-demo`。然后在里面创建 `top.sv`：

```systemverilog
module child(input logic clk, input logic rst_n, output logic done);
    assign done = rst_n & clk;
endmodule

module top(input logic clk, input logic rst_n, output logic done);
    child u_child(
        .clk(clk),
        .rst_n(rst_n),
        .done(done)
    );
endmodule
```

用 VS Code 打开这个文件夹，而不是只打开单个文件：

```powershell
code .\vizsla-demo
```

打开 `top.sv` 后，VS Code 右下角应该显示文件语言为 `SystemVerilog`。如果你打开的是 `.v` 或 `.vh`，语言会显示为 `Verilog`。

## 支持的文件后缀

Vizsla 扩展会识别这些文件：

- `.v`
- `.vh`
- `.sv`
- `.svh`
- `.svi`

如果你的文件后缀不在这个列表里，VS Code 不会自动把它交给 Vizsla。请先把文件改成常见后缀，或者在 VS Code 里手动关联语言。

## 没有配置文件时会发生什么

如果工程根目录没有 `vizsla_config.toml`，Vizsla 会把你打开的工作区根目录当作源码目录，并默认把这个目录也当作 include 目录。

这对小工程很方便。你刚开始试用时，不需要写配置文件。

> [!NOTE]
> **讨论**
>
> 当工程变大之后，常见情况会复杂一点：源码在 `rtl/`，头文件在 `include/`，仿真文件在 `tb/`，第三方库在 `ip/`。这时你就应该写 `vizsla_config.toml`，明确告诉 Vizsla 哪些文件要看、哪些目录用于 include、哪些目录要排除。
>
> 下一页我们先确认服务器是否正常工作。配置文件稍后再写。

