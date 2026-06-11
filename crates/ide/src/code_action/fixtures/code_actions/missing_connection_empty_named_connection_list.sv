//- repair: MissingConnection
module child(input a, input b); endmodule
module top; child u(/*caret*/); endmodule
