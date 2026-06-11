//- action: convert_ordered_ports
module child(input a, input b); endmodule
module top; child u(/*caret*/x, y); endmodule
