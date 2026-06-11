module m(input custom_t a); endmodule
module top; wire sig; m u0(.a(/*caret*/)); endmodule
