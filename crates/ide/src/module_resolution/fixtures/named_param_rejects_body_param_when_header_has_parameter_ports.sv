//- root: best_effort
//- query: named_param
//- focus: /project/top.sv
//- file: /project/child.sv
module child #(parameter PORT = 1) (); parameter BODY = 2; endmodule
//- file: /project/top.sv
module top; child #(./*caret*/BODY(1)) u(); endmodule
