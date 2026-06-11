//- repair: MissingParameter
module child #(parameter A, parameter B) (); endmodule
module top; child #(/*caret*/) u(); endmodule
