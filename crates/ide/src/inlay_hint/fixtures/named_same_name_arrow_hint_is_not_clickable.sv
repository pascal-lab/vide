//- config: port
module child(output clk); endmodule
module top; logic clk; child u(.clk(clk)); endmodule
