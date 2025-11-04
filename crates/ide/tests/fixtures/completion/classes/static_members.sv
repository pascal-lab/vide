class Transaction;
    static int count = 0;
    int id;
    
    function new();
        count++;
        id = count;
    endfunction
    
    static function int get_count();
        return count;
    endfunction
endclass

module test;
    initial begin
        Transaction::$0
    end
endmodule
