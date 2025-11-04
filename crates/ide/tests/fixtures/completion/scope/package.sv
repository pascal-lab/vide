package my_pkg;
    typedef logic [7:0] byte_t;
    parameter int MAX_SIZE = 256;

    function int get_max();
        return MAX_SIZE;
    endfunction
endpackage

module test;
    my_pkg::$0
endmodule
