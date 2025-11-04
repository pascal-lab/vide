package my_pkg;
    typedef logic [7:0] byte_t;
    parameter int MAX_SIZE = 256;
    
    function int get_size();
        return MAX_SIZE;
    endfunction
endpackage

module test;
    import my_pkg::*;
    
    initial begin
        byte_t data;
        int size = get_$0
    end
endmodule
