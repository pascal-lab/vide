// trigger: @
module m(input clk, input rst);
  wire en;
  always @/*caret*/(posedge clk) begin
  end
endmodule
