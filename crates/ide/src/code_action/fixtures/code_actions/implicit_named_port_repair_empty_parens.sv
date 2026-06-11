//- repair: AddImplicitNamedPortParens
module child(input a); endmodule
module top; child u(/*caret*/.a); endmodule
