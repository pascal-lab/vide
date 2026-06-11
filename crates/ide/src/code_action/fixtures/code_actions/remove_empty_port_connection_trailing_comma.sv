//- repair: RemoveEmptyPortConnections
module child(input a, input b); endmodule
module top; child u(.a(x), .b(y),/*caret*/); endmodule
