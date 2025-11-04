typedef struct packed {
    logic field1;
    logic field2;
} base_t;

typedef base_t alias_t;

module test;
    alias_t obj;

    initial begin
        obj.$0
    end
endmodule
