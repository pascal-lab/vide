module packet_fifo #(
  parameter int DEPTH = 4,
  parameter int WIDTH = 32
) (
  input  logic clk,
  input  logic rst_n,
  input  logic push,
  input  logic pop,
  input  logic [WIDTH-1:0] data_i,
  output logic [WIDTH-1:0] data_o,
  output logic full,
  output logic empty
);
  logic [$clog2(DEPTH+1)-1:0] level;

  assign full = level == DEPTH;
  assign empty = level == '0;
  assign data_o = data_i;

  always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      level <= '0;
    end else if (push && !pop) begin
      level <= level + 1'b1;
    end else if (pop && !push) begin
      level <= level - 1'b1;
    end
  end
endmodule
