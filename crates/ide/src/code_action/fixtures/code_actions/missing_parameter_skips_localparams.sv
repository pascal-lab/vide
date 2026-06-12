//- repair: MissingParameter
module child #(parameter A = 1, parameter B = 2) (); localparam L = 3; endmodule
module top; localparam L = 4; child #(/*caret*/.A(1)) u(); endmodule
