//- action: expand_named_port_connection_shorthand
module child(input a, b); endmodule
module top; logic a, b; child u(/*caret*/.a, .b); endmodule
