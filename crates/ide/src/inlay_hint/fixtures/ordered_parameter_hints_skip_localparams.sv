//- config: parameter
module child #(localparam L = 1, parameter P = 2) (); endmodule
module top; child #(3) u(); endmodule
