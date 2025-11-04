typedef struct packed {
    logic flag;
    bit enable;
    int counter;
    byte value;
    logic [15:0] wide;
} mixed_t;

module test;
    mixed_t obj;
    initial begin
        obj.$0
    end
endmodule
