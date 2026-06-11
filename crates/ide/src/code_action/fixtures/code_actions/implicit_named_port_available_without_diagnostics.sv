//- action: add_implicit_named_port_parens
module child(input a); endmodule
module top; child u(/*caret*/.a); endmodule
