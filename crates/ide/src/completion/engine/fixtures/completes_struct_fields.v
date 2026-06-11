module top;
  typedef struct {
    logic [7:0] first_field;
    logic [7:0] second_field;
  } packet_t;
  packet_t pkt;
  initial pkt./*caret*/
endmodule
