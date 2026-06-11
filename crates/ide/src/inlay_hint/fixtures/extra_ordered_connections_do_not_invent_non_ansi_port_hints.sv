//- config: port
module child(a); input a; endmodule
module top; child u(1'b0, 1'b1); endmodule
