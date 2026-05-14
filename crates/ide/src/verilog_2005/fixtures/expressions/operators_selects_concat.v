// expect-symbol: expressions

module expressions (
    input [3:0] a,
    input [3:0] b,
    output [7:0] y
);
    assign y = (&a) ? {a[1:0], b[1:0], 4'b0011} : {4{a[0]}};
endmodule
