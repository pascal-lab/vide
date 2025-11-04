typedef logic [7:0] byte_t;
typedef logic [15:0] word_t;
typedef logic [31:0] dword_t;

typedef struct packed {
    logic valid;
    byte_t data;
} packet_t;

module test;
    initial begin
        $0
    end
endmodule
