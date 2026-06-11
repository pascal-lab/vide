//- action: factor_de_morgan
module top; assign y = a == b /*caret*/&& c < d; endmodule
