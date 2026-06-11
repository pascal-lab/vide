//- action: merge_nested_if
module top; always_comb if (a) begin if (/*caret*/b) begin if (c) y = 1; end end endmodule
