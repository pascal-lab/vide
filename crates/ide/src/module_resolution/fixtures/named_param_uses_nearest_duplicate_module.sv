//- root: best_effort
//- query: named_param
//- focus: /project/a/top.sv
//- file: /project/a/child.sv
module child #(parameter WIDTH = 1) (input wire a); endmodule
//- file: /project/a/top.sv
module top; child #(./*caret*/WIDTH(1)) u(.a(sig)); endmodule
//- file: /project/b/child.sv
module child #(parameter WIDTH = 1) (input wire a); endmodule
