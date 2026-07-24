//- config: port
module child(input a, output a); endmodule
module top; logic local_a; child u(.a(local_a)); endmodule
