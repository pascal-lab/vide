//- config: port
module child(output out); endmodule
module top; logic instr_addr_o; child u(instr_addr_o); endmodule
