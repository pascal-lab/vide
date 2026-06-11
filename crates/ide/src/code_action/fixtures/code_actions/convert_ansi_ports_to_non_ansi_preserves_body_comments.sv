//- action: convert_ansi_ports_to_non_ansi
module top(/*caret*/input a, output logic b);
// keep this
assign b = a;
endmodule
