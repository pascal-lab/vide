//- config: port
module child(input a, output b, input c); endmodule
module top; logic local_b, local_c; child u(.b(local_b), .c(local_c)); endmodule
