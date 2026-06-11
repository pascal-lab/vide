//- action: reformat_number_literal
//- label: Convert 'hff0000 to 'hff_0000
module top; localparam int value = /*caret*/'hff0000; endmodule
