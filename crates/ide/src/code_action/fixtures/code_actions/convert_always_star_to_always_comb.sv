//- action: convert_always_to_always_comb
module top; logic a, y; /*caret*/always @(*) begin y = a; end endmodule
