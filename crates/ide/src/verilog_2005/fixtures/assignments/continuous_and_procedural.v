// expect-symbol: assignments
// expect-symbol: init_blk

module assignments (
    input clk,
    input d,
    output reg q
);
    assign mirror = q;

    initial begin : init_blk
        q = 1'b0;
        #1 q <= d;
        force q = 1'b1;
        release q;
    end
endmodule
