module pipeline_top (
  input  logic clk,
  input  logic in_valid,
  input  logic [15:0] in_data,
  output logic out_valid,
  output logic [15:0] out_data
);
  logic mid_valid;
  logic [15:0] mid_data;

  pipeline_stage #(
    .WIDTH(16)
  ) u_decode (
    .clk(clk),
    .valid_i(in_valid),
    .data_i(in_data),
    .valid_o(mid_valid),
    .data_o(mid_data)
  );

  pipeline_stage #(
    .WIDTH(16)
  ) u_execute (
    .clk(clk),
    .valid_i(mid_valid),
    .data_i(mid_data),
    .valid_o(out_valid),
    .data_o(out_data)
  );
endmodule
