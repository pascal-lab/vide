//- action: unwrap_single_statement_block
module top; always_comb if (a) /*caret*/begin y = 1; end endmodule
