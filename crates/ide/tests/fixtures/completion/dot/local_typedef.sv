module test;
    typedef struct packed {
        logic ready;
        logic valid;
    } status_t;

    status_t status;

    initial begin
        status.$0
    end
endmodule
