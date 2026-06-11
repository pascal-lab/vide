//- action: collapse_named_port_connection_shorthand
module child(input a, b, c); endmodule
module top; logic sw1, b, gate_out; child u(/*caret*/.a(sw1), .c(c), .b(gate_out)); endmodule
