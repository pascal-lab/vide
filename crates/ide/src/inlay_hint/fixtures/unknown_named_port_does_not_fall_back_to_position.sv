//- config: port
module child(input a); endmodule
module top; logic sig; child u(.bogus(sig)); endmodule
