//- port.clk_rst: true
//- port.io: true
module darksocv
(
    input        UART_RXD,
    output [31:0] LED,
    input  [31:0] IPORT,
    output [31:0] OPORT,
    output [3:0]  DEBUG
);
    wire [31:0] iport;
    wire [31:0] oport;

    darkio io0 (
        .RXD    (UART_RXD),
        .TXD    (UART_TXD),
        .LED    (LED),
        .IPORT  (iport),
        .OPORT  (oport),
        .DEBUG  (IODEBUG)
    );
endmodule

module darkio
(
    input         RXD,
    output        TXD,
    output [31:0] LED,
    input  [31:0] IPORT,
    output [31:0] OPORT,
    output  [3:0] DEBUG
);
endmodule
