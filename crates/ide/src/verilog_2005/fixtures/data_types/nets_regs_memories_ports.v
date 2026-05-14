// expect-symbol: data_types
// expect-symbol: mem
// expect-symbol: wildcard_ports
// expect-symbol: explicit_ansi_port

module data_types (
    input clk,
    input [7:0] din,
    output reg [7:0] dout
);
    wire [7:0] bus;
    reg [7:0] mem [0:3];
    integer idx;
    time stamp;
    real gain;

    always @(posedge clk) begin
        mem[0] <= din;
        dout <= mem[idx[1:0]];
    end
endmodule

module wildcard_ports (.*);
    input clk;
    output y;
    assign y = clk;
endmodule

module explicit_ansi_port (
    output .y(out_wire),
    input .a(in_wire)
);
    wire out_wire;
    wire in_wire;
endmodule
