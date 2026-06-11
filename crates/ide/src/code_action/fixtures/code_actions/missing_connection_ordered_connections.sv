//- repair: MissingConnection
module child(input a, input b, input c); endmodule
module top; logic b, c; child u(/*caret*/1'b0); endmodule
