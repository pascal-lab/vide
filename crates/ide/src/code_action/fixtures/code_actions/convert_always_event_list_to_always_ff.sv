//- action: convert_always_to_always_ff
module top; logic clk, d, q; always @(/*caret*/posedge clk) q <= d; endmodule
