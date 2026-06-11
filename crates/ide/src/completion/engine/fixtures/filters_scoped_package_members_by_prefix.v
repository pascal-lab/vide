package pkg;
  localparam int pkg_value = 1;
  localparam int other_value = 2;
endpackage

module top;
  localparam int value = pkg::pkg/*caret*/;
endmodule
