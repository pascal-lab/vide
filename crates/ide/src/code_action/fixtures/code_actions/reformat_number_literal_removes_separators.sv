//- action: reformat_number_literal
//- label: Remove digit separators
module top; localparam int value = /*caret*/10_000; endmodule
