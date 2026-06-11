module top;
  generate
    if (1)
      function integer f;
        input integer arg;
        integer local_value;
        begin
          local_value = arg + /*caret*/local_value;
        end
      endfunction
  endgenerate
endmodule
