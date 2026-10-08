//- root: local
//- query: named_param
//- focus: /project/top.sv
//- file: /project/top.sv
module child #(parameter PORT = 1) (); parameter BODY = 2; endmodule
module top; child #(./*caret*/BODY(1)) u(); endmodule
