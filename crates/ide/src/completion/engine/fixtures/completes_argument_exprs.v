module m;
  wire a;
  wire b;
  localparam P = 1;
  initial begin
    integer i;
    foo(/*caret*/);
  end
endmodule
