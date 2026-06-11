//- config: macro_argument
`define MAKE(width, expr) logic [width-1:0] expr
module top; `MAKE(8, /*range-start*/data_q/*range-end*/) endmodule
