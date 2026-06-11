module m #(parameter W=1) (); endmodule
module top; m #(.W(/*caret*/)) u0(); endmodule
