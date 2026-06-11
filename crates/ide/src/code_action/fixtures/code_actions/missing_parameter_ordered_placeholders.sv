//- repair: MissingParameter
module child #(parameter A, parameter B, parameter C) (); endmodule
module top; child #(/*caret*/1) u(); endmodule
