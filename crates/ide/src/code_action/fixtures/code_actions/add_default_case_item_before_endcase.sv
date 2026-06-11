//- action: add_default_case_item
module top; always_comb case (/*caret*/sel)
    1'b0: y = 0;
endcase endmodule
