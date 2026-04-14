module m #(parameter [3:0] W = 4, parameter [7:0] Z = 8) (); endmodule
module top;
localparam [3:0] P4 = 4;
localparam [7:0] P8 = 8;
m #(P/*caret*/) u0();
endmodule
