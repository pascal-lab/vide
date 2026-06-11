module m;
  initial begin
    integer outer_value;
    begin
      integer inner_value;
      inner_value = /*caret*/;
    end
  end
endmodule
