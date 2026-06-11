//- repair: ConvertOrderedPorts
module child(input a, input b); endmodule
module top; child u(/*caret*/x, .b(y)); endmodule
