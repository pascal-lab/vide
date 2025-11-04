typedef struct packed {
    logic valid;
    logic [7:0] data;
    logic done;
} packet_t;

module test;
    packet_t pkt;
    initial begin
        pkt.d$0
    end
endmodule
