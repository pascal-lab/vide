//- config: parameter
module child #(parameter P = 1) (); endmodule
module top; child #(1, 2) u(); endmodule
