//- repair: MissingParameter
module child #(localparam int L = 1, int H = 2, parameter int A = 3, int B = 4) (); endmodule
module top; child #(/*caret*/.A(1)) u(); endmodule
