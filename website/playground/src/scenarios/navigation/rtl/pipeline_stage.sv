module pipeline_stage #(
  parameter int WIDTH = 8
) (
  input  logic clk,
  input  logic valid_i,
  input  logic [WIDTH-1:0] data_i,
  output logic valid_o,
  output logic [WIDTH-1:0] data_o
);

  always_ff @(posedge clk) begin
    valid_o <= valid_i;
    data_o <= data_i;
  end
endmodule
