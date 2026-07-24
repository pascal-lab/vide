//- root: local
//- query: named_param
//- focus: /project/top.sv
//- file: /project/left.sv
module target #(parameter P = 1); endmodule
//- file: /project/right.sv
module target; endmodule
//- file: /project/top.sv
module top; target #(./*caret*/P(2)) u(); endmodule
