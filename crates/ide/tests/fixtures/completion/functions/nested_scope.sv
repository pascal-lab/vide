function int helper_func(int x);
    return x * 2;
endfunction

module test;
    function int compute(int a, int b);
        int temp;
        temp = helper_$0
        return temp;
    endfunction
endmodule
