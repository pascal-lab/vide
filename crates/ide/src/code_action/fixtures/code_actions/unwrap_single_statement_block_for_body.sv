//- action: unwrap_single_statement_block
module top; always_comb for (int i = 0; i < 4; i++) /*caret*/begin y = i; end endmodule
