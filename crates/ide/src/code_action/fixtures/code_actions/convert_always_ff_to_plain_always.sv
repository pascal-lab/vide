//- action: convert_always_ff_to_always
module top; logic clk, d, q; /*caret*/always_ff @(posedge clk) q <= d; endmodule
