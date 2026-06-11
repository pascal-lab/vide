//- action: merge_nested_if
module top; always_comb if (/*caret*/a) begin if (b) y = 1; end endmodule
