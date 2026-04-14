module m;
  wire a;
  wire b;
  localparam P = 1;
  wire out;
  assign out = a + /*caret*/;
endmodule
