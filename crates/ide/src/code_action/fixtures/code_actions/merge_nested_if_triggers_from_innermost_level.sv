//- action: merge_nested_if
module top; always_comb if (a) begin if (b) begin if (/*caret*/c) y = 1; end end endmodule
