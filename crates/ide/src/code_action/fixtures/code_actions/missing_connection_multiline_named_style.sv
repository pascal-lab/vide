//- repair: MissingConnection
module child(input a, input b, input c); endmodule
module top;
child u(
    /*caret*/.a()
);
endmodule
