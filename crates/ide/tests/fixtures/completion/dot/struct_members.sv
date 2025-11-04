typedef struct packed {
    logic valid;
    logic [7:0] data;
} packet_t;

module test;
    packet_t pkt;
    initial begin
        pkt.$0
    end
endmodule
