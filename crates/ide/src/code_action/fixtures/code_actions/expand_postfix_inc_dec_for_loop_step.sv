//- action: expand_postfix_inc_dec
module top; int i; logic y; always_comb for (i = 0; i < 4; /*caret*/i++) y = i; endmodule
