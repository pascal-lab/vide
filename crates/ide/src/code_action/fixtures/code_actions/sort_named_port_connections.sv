//- action: sort_named_port_connections
module child(input z, input a); endmodule
module top; child u(/*caret*/.a(y), .z(x)); endmodule
