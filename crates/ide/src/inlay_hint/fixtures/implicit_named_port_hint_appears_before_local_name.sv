//- config: port
module child(input clk_i); endmodule
module top; logic clk_i; child u(.clk_i,); endmodule
