class Config;
    int timeout;
    bit enable;
    
    function void set_timeout(int val);
        timeout = val;
    endfunction
    
    function int get_timeout();
        return timeout;
    endfunction
    
    task wait_cycles(int n);
        repeat(n) #10;
    endtask
endclass

module test;
    Config cfg;
    
    initial begin
        cfg = new();
        cfg.$0
    end
endmodule
