//- action: reformat_number_literal
//- label: Convert 10000 to 10_000
module top; localparam int value = /*caret*/10000; endmodule
