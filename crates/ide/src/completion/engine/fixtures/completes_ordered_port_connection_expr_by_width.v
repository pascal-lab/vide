module m(input [3:0] a, input [7:0] b); endmodule
module top;
wire [3:0] sig4;
wire [7:0] sig8;
wire sig1;
m u0(sig4, /*caret*/);
endmodule
