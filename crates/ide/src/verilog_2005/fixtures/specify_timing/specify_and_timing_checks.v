// expect-symbol: specify_timing
// expect-symbol: t_setup

module specify_timing (
    input a,
    input b,
    output y
);
    assign y = a & b;

    specify
        specparam t_setup = 1, t_hold = 2;
        (a => y) = (1, 2);
        $setuphold(posedge a, b, t_setup, t_hold);
    endspecify
endmodule
