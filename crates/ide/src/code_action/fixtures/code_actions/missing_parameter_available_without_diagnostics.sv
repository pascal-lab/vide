//- action: add_missing_parameters
module child #(parameter A = 1, parameter B) (); endmodule
module top; child #(/*caret*/.A(1)) u(); endmodule
