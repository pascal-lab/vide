//- repair: RemoveEmptyPortConnections
module child(input a, input b); endmodule
module top; child u(/*caret*/.a(x), , .b(y)); endmodule
