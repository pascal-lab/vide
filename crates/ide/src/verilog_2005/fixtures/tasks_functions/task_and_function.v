// expect-symbol: tasks_functions
// expect-symbol: drive
// expect-symbol: add1

module tasks_functions (
    input [3:0] a,
    output reg [3:0] y
);
    task drive;
        input [3:0] value;
        begin
            y = value;
        end
    endtask

    function [3:0] add1;
        input [3:0] value;
        begin
            add1 = value + 1'b1;
        end
    endfunction

    always @* begin
        drive(add1(a));
    end
endmodule
