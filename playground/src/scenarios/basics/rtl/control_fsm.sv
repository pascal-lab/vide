package control_pkg;
  typedef enum logic [1:0] {
    IDLE,
    BUSY,
    DONE
  } state_e;

  function automatic logic is_complete(state_e state);
    return state == DONE;
  endfunction
endpackage

module control_fsm (
  input  logic clk,
  input  logic rst_n,
  input  logic start,
  output control_pkg::state_e state,
  output logic done
);
  import control_pkg::*;

  state_e next_state;

  always_comb begin
    next_state = state;
    unique case (state)
      IDLE: begin
        if (start) begin
          next_state = BUSY;
        end
      end
      BUSY: begin
        next_state = DONE;
      end
      DONE: begin
        next_state = IDLE;
      end
    endcase
  end

  always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      state <= IDLE;
    end else begin
      state <= next_state;
    end
  end

  assign done = is_complete(state);
endmodule
