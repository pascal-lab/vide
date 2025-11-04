typedef struct packed {
    logic valid;
    logic [7:0] data;
} packet_t;

module test1;
    packet_t pkt1;
    initial begin
        pkt1.$0
    end
endmodule

module test2;
    packet_t pkt2;
endmodule
