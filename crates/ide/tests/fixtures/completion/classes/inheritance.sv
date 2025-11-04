class BaseTransaction;
    int id;
    bit valid;
    
    function void set_id(int new_id);
        id = new_id;
    endfunction
endclass

class DataTransaction extends BaseTransaction;
    logic [31:0] data;
    
    function void set_data(logic [31:0] new_data);
        data = new_data;
    endfunction
endclass

module test;
    initial begin
        DataTransaction trans = new();
        trans.$0
    end
endmodule
