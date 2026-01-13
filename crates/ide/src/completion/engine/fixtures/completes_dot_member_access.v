module leaf;
endmodule

module sub;
  wire inner;
  logic [3:0] data;
  leaf u1();
endmodule

module top;
  sub u0();
  initial begin
    u0./*caret*/
  end
endmodule
