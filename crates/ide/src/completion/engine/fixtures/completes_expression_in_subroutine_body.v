module top;
  function int f(input int px_in);
    int px_local;
    begin
      int px_inner;
      px_local = px/*caret*/;
    end
  endfunction
endmodule

