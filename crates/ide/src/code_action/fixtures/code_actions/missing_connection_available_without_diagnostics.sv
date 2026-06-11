//- action: add_missing_connections
//- label: Fill connections
module child(input a, input b); endmodule
module top; child u(/*caret*/.a()); endmodule
