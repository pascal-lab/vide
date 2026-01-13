module m(a);
output a;
reg [7:0] a;
endmodule
module top;
wire [7:0] sig8;
wire sig1;
m u0(.a(/*caret*/));
endmodule
