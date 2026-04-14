module m(input [3:0] a, input [7:0] b); endmodule
module top;
wire [3:0] sig4;
m u0(.a(sig4), /*caret*/);
endmodule
