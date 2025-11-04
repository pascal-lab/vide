package pkg_a;
    typedef logic [7:0] byte_t;
    parameter int SIZE_A = 8;
endpackage

package pkg_b;
    typedef logic [15:0] word_t;
    parameter int SIZE_B = 16;
endpackage

module test;
    import pkg_a::*;
    import pkg_b::*;
    
    initial begin
        $0
    end
endmodule
