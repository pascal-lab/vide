module counter_tb;
  logic clk;
  logic rst_n;
  logic [7:0] value;

  counter dut (
    .clk(clk),
    .rst_n(rst_n),
    .value(value)
  );
endmodule
