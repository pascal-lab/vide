//- action: collapse_named_port_connection_shorthand
module child(input a); endmodule
module top; logic a; child u(/*caret*/.a(a)); endmodule
