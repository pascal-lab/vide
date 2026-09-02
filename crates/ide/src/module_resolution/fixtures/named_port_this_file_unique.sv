//- root: local
//- query: named_port
//- focus: /project/top.sv
//- file: /project/top.sv
module child(input wire a); endmodule
module top; logic sig; child u(./*caret*/a(sig)); endmodule
