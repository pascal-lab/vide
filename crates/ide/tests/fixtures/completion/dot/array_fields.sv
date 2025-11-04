typedef struct packed {
    logic [3:0] nibble;
    logic [31:0] bytes;
    logic single;
} array_struct_t;

module test;
    array_struct_t obj;

    initial begin
        obj.$0
    end
endmodule
