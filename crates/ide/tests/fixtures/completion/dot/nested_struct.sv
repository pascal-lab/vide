typedef struct packed {
    logic [7:0] addr;
    logic [7:0] data;
} header_t;

typedef struct packed {
    header_t hdr;
    logic [15:0] payload;
} packet_t;

module test;
    packet_t pkt;
    initial begin
        pkt.$0
    end
endmodule
