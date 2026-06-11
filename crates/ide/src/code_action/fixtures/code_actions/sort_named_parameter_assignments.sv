//- action: sort_named_parameter_assignments
module child #(parameter WIDTH = 8, parameter DEPTH = 16) (); endmodule
module top; child #(/*caret*/.DEPTH(16), .WIDTH(8)) u(); endmodule
