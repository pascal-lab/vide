//- action: merge_nested_if
module top; always_comb if (/*caret*/a) begin if (b) begin if (c) y = 1; end end endmodule
