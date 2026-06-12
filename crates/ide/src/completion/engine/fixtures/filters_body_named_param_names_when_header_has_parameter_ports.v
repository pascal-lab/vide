module child #(parameter A = 1) (); parameter B = 2; endmodule
module top; child #(./*caret*/) u(); endmodule
