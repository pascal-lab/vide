module m #(parameter W=1) (); endmodule
module top; m #(./*caret*/W(1)) u0(); endmodule
