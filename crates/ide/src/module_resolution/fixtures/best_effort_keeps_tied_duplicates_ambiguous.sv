//- root: best_effort
//- query: module child
//- focus: /project/top.sv
//- file: /project/a/child.sv
module child; endmodule
//- file: /project/b/child.sv
module child; endmodule
//- file: /project/top.sv
module top; child u(); endmodule
