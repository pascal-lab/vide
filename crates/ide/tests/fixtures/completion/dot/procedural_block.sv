typedef struct packed {
    logic start;
    logic stop;
} control_t;

module test;
    initial begin
        control_t ctrl;
        ctrl.$0
    end
endmodule
