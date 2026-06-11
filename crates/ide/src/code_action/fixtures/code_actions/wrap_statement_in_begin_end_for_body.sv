//- action: wrap_statement_in_begin_end
module top; always_comb for (int i = 0; i < 4; i++) /*caret*/y = i; endmodule
