//- config: system_call
module m;
  logic [7:0] mem [0:15];
  initial begin
    $display("x=%d", x);
    $readmemh("mem.hex", mem);
  end
endmodule
