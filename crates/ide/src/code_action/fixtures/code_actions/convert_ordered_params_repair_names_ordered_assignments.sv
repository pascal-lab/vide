//- repair: ConvertOrderedParams
module child #(parameter A = 1, parameter B = 2) (); endmodule
module top; child #(/*caret*/8, .B(16)) u(); endmodule
