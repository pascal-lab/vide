//- root: local
//- query: named_param
//- focus: /project/top.sv
//- file: /project/top.sv
module target #(parameter P = 1); endmodule
module target; endmodule
module top; target #(./*caret*/P(2)) u(); endmodule
