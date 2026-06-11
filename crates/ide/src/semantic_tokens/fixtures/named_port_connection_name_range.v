//- port.io: true
module child(output logic instr_req_o);
endmodule

module top(output logic instr_req_o);
child u_child (
    .instr_req_o (instr_req_o),
);
endmodule
