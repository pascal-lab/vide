//- action: extract_variable
module top; logic [3:0] b; logic [7:0] a, y; always_comb begin y = /*selection*/b + a/*selection*/; end endmodule
