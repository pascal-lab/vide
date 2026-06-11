//- config: port
module child(.out(foo)); output foo; endmodule
module top; logic sig; child u(.out(sig)); endmodule
