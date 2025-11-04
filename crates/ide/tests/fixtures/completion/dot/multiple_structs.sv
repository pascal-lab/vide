typedef struct packed {
    logic a;
    logic b;
} type1_t;

typedef struct packed {
    logic x;
    logic y;
    logic z;
} type2_t;

module test;
    type1_t obj1;
    type2_t obj2;

    initial begin
        obj2.$0
    end
endmodule
