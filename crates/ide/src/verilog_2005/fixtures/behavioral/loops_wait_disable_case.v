// expect-symbol: behavioral
// expect-symbol: seq

module behavioral (
    input clk,
    input rst_n,
    input en,
    input [1:0] sel,
    output reg [3:0] q
);
    integer i;

    always @(posedge clk or negedge rst_n) begin : seq
        if (!rst_n) begin
            q <= 4'b0;
        end else begin
            for (i = 0; i < 4; i = i + 1)
                q[i] <= en;

            casez (sel)
                2'b0?: q <= 4'h1;
                2'b10: q <= 4'h2;
                default: q <= 4'hf;
            endcase

            wait (en) q <= q + 1'b1;
            disable seq;
        end
    end
endmodule
