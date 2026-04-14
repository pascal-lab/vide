module leaf;
  wire leaf_wire;
endmodule

module sub;
  wire inner;
  wire [3:0] data;
  leaf u1();
endmodule

module top;
  sub u0 [0:1] ();
  initial begin
    u0[0]./*caret*/
  end
endmodule
