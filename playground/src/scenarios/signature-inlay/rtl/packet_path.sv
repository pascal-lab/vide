module packet_path (
  input  logic clk,
  input  logic rst_n,
  input  logic push,
  input  logic pop,
  input  logic [31:0] data_i,
  output logic [31:0] data_o,
  output logic stalled
);
  logic full;
  logic empty;

  packet_fifo #(8, 32) u_packet_fifo (
    clk,
    rst_n,
    push,
    pop,
    data_i,
    data_o,
    full,
    empty
  );

  assign stalled = full || empty;
endmodule
