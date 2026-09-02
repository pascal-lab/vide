//- root: local
//- query: named_param
//- focus: /project/top.sv
//- file: /project/top.sv
module child #(parameter WIDTH = 1); endmodule
module top; child #(./*caret*/WIDTH(1)) u(); endmodule
