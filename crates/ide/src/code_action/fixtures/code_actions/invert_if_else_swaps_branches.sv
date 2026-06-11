//- action: invert_if_else
module top; always_comb if (/*caret*/a) y = 1; else y = 0; endmodule
