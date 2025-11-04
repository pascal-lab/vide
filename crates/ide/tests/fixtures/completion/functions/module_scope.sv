module test;
    function int add(int a, int b);
        return a + b;
    endfunction
    
    function logic [7:0] multiply(logic [7:0] x, logic [7:0] y);
        return x * y;
    endfunction
    
    task wait_cycles(int n);
        repeat(n) #10;
    endtask
    
    initial begin
        int result;
        result = $0
    end
endmodule
