//- action: convert_literal_base
//- label: Convert literal to binary
module top; localparam logic [7:0] value = /*caret*/8'sh2A; endmodule
