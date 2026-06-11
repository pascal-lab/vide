//- repair: MissingParameter
module child #(parameter A, parameter B, parameter C) (); endmodule
module top; parameter B = 2; parameter C = 3; child #(/*caret*/1) u(); endmodule
