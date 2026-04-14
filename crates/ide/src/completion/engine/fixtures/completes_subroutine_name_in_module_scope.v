module top;
  function int px_func(input int a);
    px_func = a;
  endfunction

  int px_var;
  assign px_var = px_f/*caret*/;
endmodule

