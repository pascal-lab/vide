typedef struct packed {
    logic a;
    logic b;
    logic c;
} abc_t;

module test;
    abc_t obj;

    initial begin
        obj = '0;
        obj.$0
    end
endmodule
