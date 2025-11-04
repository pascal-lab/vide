typedef struct packed {
    logic clk;
    logic rst;
    logic en;
} signals_t;

module test;
    signals_t sigs;

    always @(posedge sigs.clk) begin
        sigs.$0
    end
endmodule
