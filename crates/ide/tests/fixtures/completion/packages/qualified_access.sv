package config_pkg;
    typedef enum {IDLE, ACTIVE, DONE} state_t;
    parameter int TIMEOUT = 1000;
    
    class Config;
        int timeout = TIMEOUT;
    endclass
endpackage

module test;
    initial begin
        config_pkg::$0
    end
endmodule
