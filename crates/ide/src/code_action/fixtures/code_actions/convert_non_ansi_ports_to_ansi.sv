//- action: convert_non_ansi_ports_to_ansi
module top(/*caret*/a, b);
input wire a;
output logic b;
assign b = a;
endmodule
