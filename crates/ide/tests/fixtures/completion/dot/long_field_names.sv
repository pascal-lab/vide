typedef struct packed {
    logic transmission_enable;
    logic reception_ready;
    logic [31:0] accumulated_error_count;
} long_names_t;

module test;
    long_names_t cfg_reg;

    initial begin
        cfg_reg.$0
    end
endmodule
