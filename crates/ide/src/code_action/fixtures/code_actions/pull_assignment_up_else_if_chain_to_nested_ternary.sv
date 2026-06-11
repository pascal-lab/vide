//- action: pull_assignment_up
module top; always_comb if (/*caret*/a) y = 1; else if (b) y = 2; else y = 3; endmodule
