module top;
  generate
    if (1) begin : g
      localparam int generated_value = 1;
      initial begin
        int local_value;
        local_value = gen/*caret*/;
      end
    end
  endgenerate
endmodule
