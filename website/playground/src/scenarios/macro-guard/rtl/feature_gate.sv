`include "feature_defs.svh"

module feature_gate(input logic clk, output logic pulse);
`ifdef VIDE_LAB_ENABLE
  always_ff @(posedge clk) begin
    pulse <= ~pulse;
  end
`else
  assign pulse = 1'b0;
`endif
endmodule
