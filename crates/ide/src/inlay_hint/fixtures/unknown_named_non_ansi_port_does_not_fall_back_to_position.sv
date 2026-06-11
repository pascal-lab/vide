//- config: port
module child(a); input a; endmodule
module top; logic sig; child u(.bogus(sig)); endmodule
