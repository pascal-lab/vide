// trigger: ,
module m(input a, input b); endmodule
module top;
  wire x;
  m u0(.a(x), /*caret*/);
endmodule