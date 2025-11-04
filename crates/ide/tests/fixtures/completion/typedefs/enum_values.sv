typedef enum logic [1:0] {
    IDLE = 2'b00,
    ACTIVE = 2'b01,
    DONE = 2'b10,
    ERROR = 2'b11
} state_t;

module test;
    state_t current_state;
    
    initial begin
        current_state = $0
    end
endmodule
