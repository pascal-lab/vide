//- config: port
module child(input i, output o, inout io, ref r); endmodule
module top; logic a, b, c, d; child u(a, b, c, d); endmodule
