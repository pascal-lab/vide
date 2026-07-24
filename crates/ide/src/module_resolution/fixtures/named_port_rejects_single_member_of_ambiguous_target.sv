//- root: local
//- query: named_port
//- focus: /project/top.sv
//- file: /project/left.sv
module target(input a); endmodule
//- file: /project/right.sv
module target; endmodule
//- file: /project/top.sv
module top; logic x; target u(./*caret*/a(x)); endmodule
