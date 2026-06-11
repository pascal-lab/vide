module m;
  function int same_type(input int a);
    same_type = a;
  endfunction
  function logic [15:0] wrong_type(input logic [15:0] a);
    wrong_type = a;
  endfunction

  int lhs;
  assign lhs = /*caret*/;
endmodule
