class packet_c;
    logic [7:0] data;
    logic valid;
    int id;
endclass

module test;
    packet_c pkt;
    logic [7:0] tmp;
    
    initial begin
        pkt = new();
        if (pkt.$0) begin
        end
    end
endmodule