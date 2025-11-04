// Port connection: filter out already connected ports
module adder(
    input logic clk,
    input logic rst_n,
    input logic [7:0] a,
    input logic [7:0] b,
    output logic [8:0] sum
);
endmodule

module top;
    logic clock;
    logic reset;
    logic [7:0] data_a;
    logic [7:0] data_b;
    logic [8:0] result;
    
    adder inst(
        .clk(clock),
        .rst_n(reset),
        .$0
    );
endmodule
