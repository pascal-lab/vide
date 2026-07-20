module rename_playground (
  input  logic clk,
  input  logic load,
  input  logic [7:0] sample_i,
  output logic [7:0] sample_o,
  output logic sample_ready
);
  logic [7:0] buffered_sample;

  always_ff @(posedge clk) begin
    if (load) begin
      buffered_sample <= sample_i;
    end
    sample_o <= buffered_sample;
  end

  assign sample_ready = |buffered_sample;
endmodule
