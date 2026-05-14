// expect-symbol: system_tasks
// expect-symbol: log

module system_tasks;
    integer log;

    initial begin
        log = 0;
        $display("log=%0d", log);
        $finish;
    end
endmodule
