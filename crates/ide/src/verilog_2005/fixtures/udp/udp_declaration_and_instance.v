// expect-symbol: udp_mux
// expect-symbol: uses_udp
// expect-symbol: u_udp

primitive udp_mux (out, sel, a, b);
    output out;
    input sel, a, b;
    table
        0 0 1 : 1;
        1 1 0 : 1;
        ? ? ? : 0;
    endtable
endprimitive

module uses_udp (
    input sel,
    input a,
    input b,
    output y
);
    udp_mux u_udp (y, sel, a, b);
endmodule
