//- action: merge_nested_if
module top; always_comb if (/*caret*/a || b) begin if (c || d) y = 1; end endmodule
