package outer_pkg;
    package inner_pkg;
        typedef logic [7:0] byte_t;
        parameter int INNER_VALUE = 42;
    endpackage

    parameter int OUTER_VALUE = 100;
endpackage

module test;
    outer_pkg::inner_pkg::$0
endmodule
