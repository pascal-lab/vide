//- action: convert_non_ansi_ports_to_ansi
module top(/*caret*/a, b);
// keep first
input wire a;
// keep second
output logic b;
assign b = a;
endmodule
