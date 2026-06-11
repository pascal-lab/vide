//- repair: MissingParameter
module child #(parameter A = 1, parameter B, parameter C) (); endmodule
module top;
child #(
    /*caret*/.A(1),
) u();
endmodule
