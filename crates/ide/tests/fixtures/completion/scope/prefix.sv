package my_pkg;
    parameter int MAX_VALUE = 100;
    parameter int MIN_VALUE = 0;
    parameter string MODE = "fast";
endpackage

module test;
    int x = my_pkg::M$0
endmodule
