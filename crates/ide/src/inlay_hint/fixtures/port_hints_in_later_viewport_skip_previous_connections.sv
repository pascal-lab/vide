//- config: port
module child(input a, output b); endmodule
module top; logic local_a, local_b; child u(.a(local_a), /*range-start*/.b(local_b)/*range-end*/); endmodule
