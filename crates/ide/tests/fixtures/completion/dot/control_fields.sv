typedef struct packed {
    logic mode;
    logic [7:0] cfg;
    logic status;
} ctrl_t;

module test;
    ctrl_t control;

    initial begin
        control.$0
    end
endmodule
