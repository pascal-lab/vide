//- action: collapse_compound_assignment
module top; always_comb begin /*caret*/a = a + b; end endmodule
