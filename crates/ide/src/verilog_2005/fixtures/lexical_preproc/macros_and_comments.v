// expect-symbol: lexical_preproc
`define V2005_WIDTH 7

module lexical_preproc (
    input [`V2005_WIDTH:0] a,
    output [`V2005_WIDTH:0] y
);
    // region-style comments should not affect model construction.
    assign y = a;
endmodule
