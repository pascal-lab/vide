//- repair: MissingConnection
module child(input a, input b, input c); endmodule
module top; child u(/*caret*/1'b0); endmodule
