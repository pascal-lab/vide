typedef struct packed {
    logic error;
    logic warning;
} flags_t;

module test;
    function void check_flags();
        flags_t f;
        f.$0
    endfunction
endmodule
