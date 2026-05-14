// expect-symbol: child
// expect-symbol: top
// expect-symbol: i
// expect-symbol: g_loop
// expect-symbol: cfg_top

module child (
    input a,
    output y
);
    assign y = a;
endmodule

module top (
    input [3:0] a,
    output [3:0] y
);
    genvar i;

    generate
        for (i = 0; i < 4; i = i + 1) begin : g_loop
            child u_child (.a(a[i]), .y(y[i]));
        end
    endgenerate
endmodule

config cfg_top;
    design work.top;
    default liblist work;
endconfig

library work "*.v";
