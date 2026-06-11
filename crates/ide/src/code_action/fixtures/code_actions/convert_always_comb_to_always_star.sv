//- action: convert_always_comb_to_always
module top; logic a, y; /*caret*/always_comb begin y = a; end endmodule
