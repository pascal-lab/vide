typedef struct packed {
    logic valid;
    logic [7:0] data;
} packet_t;

module test;
    packet_t pkt;
    logic clk, rst;
    int counter;

    initial begin
        $0
    end
endmodule
