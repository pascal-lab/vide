//- action: apply_de_morgan
module top; assign y = /*caret*/!(a == b || c != d || e <= f); endmodule
