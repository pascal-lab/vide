//- action: pull_assignment_up
module top; always_comb if (a ? b : c) /*caret*/y = 1; else y = 0; endmodule
