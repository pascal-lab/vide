// trigger: ,
// when declaring new symbol, after typing the type, the completion should not suggest anything

// when in trivia and string literals, no completion should be suggested

// those keywords complete in modules (input, etc) should also be suggested in tasks and functions

// ,,/*caret*/

`timescale 1ns / 1ps

module adder (
    input  [3:0] a,
    input  [3:0] b,
    output [4:0] y
);
endmodule
