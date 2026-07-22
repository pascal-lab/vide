module counter #(
  parameter int WIDTH = 8
) (
  input  logic clk,
  input  logic rst_n,
  output logic [WIDTH-1:0] value
);

  always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      value <= '0;
    end else begin
      value <= value + 1'b1;
    end
  end
endmodule
