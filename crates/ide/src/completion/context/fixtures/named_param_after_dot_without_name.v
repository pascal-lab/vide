module m #(parameter W=1) (); endmodule
module top; m #(./*caret*/) u0(); endmodule
