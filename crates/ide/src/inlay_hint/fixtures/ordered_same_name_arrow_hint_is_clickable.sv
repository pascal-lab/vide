//- config: port
module child(output instr_addr_o); endmodule
module top; logic instr_addr_o; child u(instr_addr_o); endmodule
