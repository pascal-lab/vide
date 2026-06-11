//- action: convert_ordered_params
module child #(parameter A = 1, parameter B = 2) (); endmodule
module top; child #(/*caret*/8, 16) u(); endmodule
