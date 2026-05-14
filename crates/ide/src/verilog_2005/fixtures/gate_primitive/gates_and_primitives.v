// expect-symbol: gates_and_primitives
// expect-symbol: u_and

module gates_and_primitives (
    input a,
    input b,
    output y
);
    and u_and (y, a, b);
    pullup p_pull (y);
endmodule
