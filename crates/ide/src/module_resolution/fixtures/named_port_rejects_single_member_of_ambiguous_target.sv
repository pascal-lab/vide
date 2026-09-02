//- root: local
//- query: named_port
//- focus: /project/top.sv
//- file: /project/top.sv
module target(input a); endmodule
module target; endmodule
module top; logic x; target u(./*caret*/a(x)); endmodule
