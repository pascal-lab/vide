//- action: pull_assignment_up
module top; always_comb if (a) y = 1; else if (b) /*caret*/y = 2; else y = 3; endmodule
